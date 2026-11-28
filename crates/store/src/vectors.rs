//! # Semantic vector index
//!
//! Embeddings of the text a source's content text index already holds, kept
//! beside it in its own SQLite file, `vectors.db`. Like `text.db` it is part
//! of the store's generation: every vector is computed from rows in
//! `text.db`, so dropping the file loses nothing a pass cannot compute again.
//!
//! A document is one row of the text index: a file's extracted text, a
//! recording's transcript, or what a sidecar says some content shows (scene
//! and caption descriptions). Each document is cut into chunks
//! ([`chunk_text`], [`chunk_spans`]) and each chunk gets one vector. A
//! chunk stores where its text lives in `text.db` rather than the text, so
//! the file holds only vectors.
//!
//! The work queue is the join between the two files. A document is pending
//! when `vec_doc` has no row for it with the stamp it carries now, and the
//! stamp changes whenever the text row is rewritten. Only documents inside
//! the source's deep scope are pending, so a released folder costs nothing.
//!
//! Vectors are stored as signed bytes with one scale per vector. The
//! embedders Spacedrive ships produce unit vectors, where eight bits keep
//! the ranking of a cosine search and take a quarter of the space of floats.
//! Search is exhaustive over a matrix cached in memory: at the sizes a deep
//! scope holds, a scan of every vector takes milliseconds and needs no
//! approximate index or native extension.
//!
//! [`hybrid_search`] fuses this ranking with the BM25 ranking of the text
//! index, so a query that names an exact word still finds it, and a query
//! that describes something finds files that never use its words.
//!
//! ## Example
//! ```rust,no_run
//! # async fn example(dir: &std::path::Path, embed: impl Fn(&str) -> Vec<f32>) -> sd_store::Result<()> {
//! use sd_store::{ContentTextIndex, DeepScope, HybridSearch, Origin, VectorIndex};
//!
//! let text = ContentTextIndex::open(dir).await?;
//! let vectors = VectorIndex::open(dir).await?;
//! vectors.use_model("multilingual-e5-small", 384).await?;
//! let scope = DeepScope::whole();
//! for doc in vectors.pending(&scope, (Origin::Text, 0), 16).await? {
//!     let chunks = vectors.chunks(&doc).await?;
//!     let embedded: Vec<Vec<f32>> = chunks.iter().map(|c| embed(&c.text)).collect();
//!     vectors.write(&doc, &chunks, &embedded).await?;
//! }
//! let hits = sd_store::hybrid_search(&text, &vectors, &HybridSearch {
//!     query: "kitchen renovation budget".into(),
//!     vector: embed("kitchen renovation budget"),
//!     scope,
//!     limit: 20,
//!     ..Default::default()
//! })
//! .await?;
//! # Ok(())
//! # }
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::sync::Arc;
use std::time::Duration;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions};
use sqlx::{Connection, Row};
use uuid::Uuid;

use crate::content_text::{
	any_word_expression, ContentTextIndex, SnippetPart, TextSearch, RECORD_PATH, TEXT_INDEX_FILE,
	TRANSCRIPT_KIND,
};
use crate::deep_scope::DeepScope;
use crate::error::{Error, Result};
use crate::file::address;

/// The sidecar's file name inside a source's directory.
pub const VECTOR_INDEX_FILE: &str = "vectors.db";

/// Characters a text chunk aims for. About 250 English tokens, well inside
/// the 512 a small sentence embedder reads.
pub const CHUNK_CHARS: usize = 1000;
/// Characters two neighbouring text chunks share, so a sentence cut at a
/// boundary is still whole in one of them.
pub const CHUNK_OVERLAP: usize = 150;
/// The most chunks one document gets. Text past it is still found by the
/// word index; this bounds what one long PDF costs to embed.
pub const MAX_CHUNKS_PER_DOC: usize = 64;

/// BM25 and vector rankings are fused with this constant, the value the
/// reciprocal rank fusion paper found robust across collections.
const FUSION_K: f64 = 60.0;

const SCHEMA: [&str; 4] = [
	"CREATE TABLE IF NOT EXISTS vec_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)",
	"CREATE TABLE IF NOT EXISTS vec_doc (
    origin INTEGER NOT NULL,
    doc_id INTEGER NOT NULL,
    stamp TEXT NOT NULL,
    PRIMARY KEY (origin, doc_id)
) WITHOUT ROWID",
	"CREATE TABLE IF NOT EXISTS vec_chunk (
    id INTEGER PRIMARY KEY,
    origin INTEGER NOT NULL,
    doc_id INTEGER NOT NULL,
    a INTEGER NOT NULL,
    b INTEGER NOT NULL,
    start_ms INTEGER,
    end_ms INTEGER,
    scale REAL NOT NULL,
    vector BLOB NOT NULL
)",
	"CREATE INDEX IF NOT EXISTS vec_chunk_doc ON vec_chunk (origin, doc_id)",
];

/// Which table of `text.db` a document is a row of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Origin {
	/// `text_doc`: a file's extracted text.
	Text = 0,
	/// `transcript_doc`: what a recording says, in timed spans.
	Transcript = 1,
	/// `description_doc`: what a scene or caption sidecar says.
	Description = 2,
}

impl Origin {
	fn from_i64(value: i64) -> Option<Self> {
		match value {
			0 => Some(Self::Text),
			1 => Some(Self::Transcript),
			2 => Some(Self::Description),
			_ => None,
		}
	}
}

/// A document whose vectors are missing or were made from other text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingVectors {
	pub origin: Origin,
	/// The row id in its `text.db` table.
	pub doc_id: i64,
	/// What the row says now. Written back with the vectors, so the
	/// document leaves the queue until its row is rewritten.
	pub stamp: String,
	/// The extractor that read it, `transcript`, or the description kind.
	pub kind: String,
}

/// One piece of a document, embedded on its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk {
	/// For text, the offset of the chunk in characters; for spans, the id of
	/// the first span.
	pub a: i64,
	/// For text, the length in characters; for spans, the id of the last.
	pub b: i64,
	pub start_ms: Option<i64>,
	pub end_ms: Option<i64>,
	/// What is embedded. Not stored.
	pub text: String,
}

/// A timed piece of text as `text.db` holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
	pub id: i64,
	pub start_ms: Option<i64>,
	pub end_ms: Option<i64>,
	pub text: String,
}

/// Cut a document's text into chunks of about `target` characters that
/// overlap by `overlap`, ending each at the strongest break near its end: a
/// blank line, then a line end, then a sentence end, then a space. At most
/// `max_chunks` are returned, from the start.
pub fn chunk_text(text: &str, target: usize, overlap: usize, max_chunks: usize) -> Vec<Chunk> {
	let chars: Vec<char> = text.chars().collect();
	let target = target.max(1);
	let overlap = overlap.min(target / 2);
	let mut chunks = Vec::new();
	let mut start = 0;
	while start < chars.len() && chunks.len() < max_chunks {
		let mut end = (start + target).min(chars.len());
		if end < chars.len() {
			end = best_break(&chars, start + target / 2, end).unwrap_or(end);
		}
		let piece: String = chars[start..end].iter().collect();
		if !piece.trim().is_empty() {
			chunks.push(Chunk {
				a: start as i64,
				b: (end - start) as i64,
				start_ms: None,
				end_ms: None,
				text: piece.trim().to_string(),
			});
		}
		if end >= chars.len() {
			break;
		}
		let mut next = end.saturating_sub(overlap).max(start + 1);
		// Start the next chunk on a word, not inside one.
		while next < end && !chars[next - 1].is_whitespace() {
			next += 1;
		}
		start = next;
	}
	chunks
}

/// The position just after the strongest break in `chars[from..to]`.
fn best_break(chars: &[char], from: usize, to: usize) -> Option<usize> {
	let window = from..to;
	let after =
		|pred: &dyn Fn(usize) -> bool| window.clone().rev().find(|&i| pred(i)).map(|i| i + 1);
	after(&|i| chars[i] == '\n' && i > 0 && chars[i - 1] == '\n')
		.or_else(|| after(&|i| chars[i] == '\n'))
		.or_else(|| {
			after(&|i| {
				matches!(chars[i], '.' | '!' | '?' | '。' | '！' | '？')
					&& chars.get(i + 1).is_none_or(|c| c.is_whitespace())
			})
		})
		.or_else(|| after(&|i| chars[i].is_whitespace()))
}

/// Group consecutive spans into chunks of about `target` characters, each
/// timed from its first span's start to its last span's end. A span is never
/// split, and at most `max_chunks` are returned.
pub fn chunk_spans(spans: &[Span], target: usize, max_chunks: usize) -> Vec<Chunk> {
	let mut chunks: Vec<Chunk> = Vec::new();
	let mut current: Option<Chunk> = None;
	for span in spans {
		let text = span.text.trim();
		if text.is_empty() {
			continue;
		}
		if let Some(chunk) = current.as_mut() {
			if chunk.text.chars().count() + text.chars().count() < target {
				chunk.text.push(' ');
				chunk.text.push_str(text);
				chunk.b = span.id;
				chunk.end_ms = span.end_ms.or(chunk.end_ms);
				continue;
			}
			chunks.extend(current.take());
			if chunks.len() >= max_chunks {
				return chunks;
			}
		}
		current = Some(Chunk {
			a: span.id,
			b: span.id,
			start_ms: span.start_ms,
			end_ms: span.end_ms,
			text: text.to_string(),
		});
	}
	chunks.extend(current);
	chunks.truncate(max_chunks);
	chunks
}

/// A unit vector as signed bytes and the scale that maps them back.
pub fn quantize(vector: &[f32]) -> (f32, Vec<u8>) {
	let max = vector.iter().fold(0.0f32, |m, v| m.max(v.abs()));
	if max == 0.0 || !max.is_finite() {
		return (0.0, vec![0; vector.len()]);
	}
	let scale = max / 127.0;
	let bytes = vector
		.iter()
		.map(|v| (v / scale).round().clamp(-127.0, 127.0) as i8 as u8)
		.collect();
	(scale, bytes)
}

/// The dot product of a float query with a quantized vector.
fn dot_quantized(query: &[f32], scale: f32, bytes: &[i8]) -> f32 {
	query
		.iter()
		.zip(bytes)
		.map(|(q, b)| q * f32::from(*b))
		.sum::<f32>()
		* scale
}

/// Every vector in the file, laid out for a scan.
struct Matrix {
	generation: u64,
	dim: usize,
	ids: Vec<i64>,
	scales: Vec<f32>,
	data: Vec<i8>,
}

/// What a source's vectors hold.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VectorIndexCounts {
	/// Documents with vectors.
	pub documents: u64,
	pub chunks: u64,
	/// The embedder the vectors came from, when any were written.
	pub model: Option<String>,
}

/// One file a vector search found.
#[derive(Debug, Clone)]
pub struct VectorHit {
	pub record_uuid: Uuid,
	/// Relative to the source root.
	pub external_id: String,
	pub kind: String,
	/// Cosine similarity of the best chunk.
	pub similarity: f32,
	/// The best chunk's text.
	pub text: String,
	pub start_ms: Option<i64>,
	pub end_ms: Option<i64>,
}

/// A search over the vectors of one source.
#[derive(Debug, Clone, Default)]
pub struct VectorSearch {
	pub vector: Vec<f32>,
	/// Only files at or under this path, relative to the source root.
	pub under: String,
	/// The source's deep scope. Vectors outside it are never returned, even
	/// before a pass prunes them.
	pub scope: DeepScope,
	/// Only these kinds. Empty means every kind.
	pub kinds: Vec<String>,
	/// Files to return.
	pub limit: usize,
}

/// The vector index of one source.
pub struct VectorIndex {
	pool: SqlitePool,
	path: PathBuf,
	/// Serializes writes, as the text index does.
	writer: tokio::sync::Mutex<()>,
	/// Raised by every write, so a search reloads the matrix it caches.
	generation: AtomicU64,
	matrix: tokio::sync::Mutex<Option<Arc<Matrix>>>,
}

impl std::fmt::Debug for VectorIndex {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("VectorIndex")
			.field("path", &self.path)
			.finish()
	}
}

impl VectorIndex {
	/// Open the index in `source_dir`, creating it on first use, with the
	/// record store and the text index attached for reads. The text index
	/// must exist; open it first.
	pub async fn open(source_dir: &Path) -> Result<Self> {
		let path = source_dir.join(VECTOR_INDEX_FILE);
		let attach = |name: &str| -> Result<String> {
			let file = source_dir.join(name);
			if !file.exists() {
				return Err(Error::SourceNotFound(file.display().to_string()));
			}
			file.to_str()
				.map(str::to_string)
				.ok_or_else(|| Error::Other(format!("path is not UTF-8: {}", file.display())))
		};
		let (store, text) = (attach("data.db")?, attach(TEXT_INDEX_FILE)?);

		let options = SqliteConnectOptions::from_str(&format!("sqlite:{}", path.display()))
			.map_err(|e| Error::Other(format!("invalid database path: {e}")))?
			.create_if_missing(true)
			.journal_mode(SqliteJournalMode::Wal)
			.busy_timeout(Duration::from_secs(5));
		let pool = SqlitePoolOptions::new()
			.max_connections(2)
			.after_release(|conn, _meta| Box::pin(async move { Ok(!conn.is_in_transaction()) }))
			.after_connect(move |conn, _meta| {
				let (store, text) = (store.clone(), text.clone());
				Box::pin(async move {
					sqlx::query("ATTACH DATABASE ? AS src")
						.bind(store)
						.execute(&mut *conn)
						.await?;
					sqlx::query("ATTACH DATABASE ? AS txt")
						.bind(text)
						.execute(&mut *conn)
						.await?;
					Ok(())
				})
			})
			.connect_with(options)
			.await?;
		let mut tx = pool.begin().await?;
		for statement in SCHEMA {
			sqlx::query(statement).execute(&mut *tx).await?;
		}
		tx.commit().await?;
		Ok(Self {
			pool,
			path,
			writer: Default::default(),
			generation: AtomicU64::new(1),
			matrix: Default::default(),
		})
	}

	/// The sidecar's file.
	pub fn path(&self) -> &Path {
		&self.path
	}

	/// Close every connection. The file stays.
	pub async fn close(&self) {
		self.pool.close().await;
	}

	/// Record which embedder the vectors come from. Vectors from another
	/// model or of another size cannot be compared with its queries, so a
	/// change drops every vector and the next pass embeds everything again.
	/// Returns whether anything was dropped.
	pub async fn use_model(&self, model: &str, dim: usize) -> Result<bool> {
		let wanted = format!("{model}:{dim}");
		let _writer = self.writer.lock().await;
		let current: Option<String> =
			sqlx::query_scalar("SELECT value FROM vec_meta WHERE key = 'model'")
				.fetch_optional(&self.pool)
				.await?;
		if current.as_deref() == Some(wanted.as_str()) {
			return Ok(false);
		}
		let mut tx = self.pool.begin().await?;
		let dropped = sqlx::query("DELETE FROM vec_chunk")
			.execute(&mut *tx)
			.await?
			.rows_affected();
		sqlx::query("DELETE FROM vec_doc").execute(&mut *tx).await?;
		sqlx::query(
			"INSERT INTO vec_meta (key, value) VALUES ('model', ?) \
			 ON CONFLICT (key) DO UPDATE SET value = excluded.value",
		)
		.bind(&wanted)
		.execute(&mut *tx)
		.await?;
		tx.commit().await?;
		self.generation.fetch_add(1, AtomicOrdering::SeqCst);
		Ok(dropped > 0)
	}

	/// Documents in `scope` after `after` whose vectors are missing or
	/// stale, in `(origin, doc_id)` order.
	pub async fn pending(
		&self,
		scope: &DeepScope,
		after: (Origin, i64),
		limit: usize,
	) -> Result<Vec<PendingVectors>> {
		let sql = format!(
			"SELECT v.origin, v.doc_id, v.stamp, v.kind FROM ({}) v \
			 WHERE (v.origin, v.doc_id) > (?, ?) AND {STALE} \
			 ORDER BY v.origin, v.doc_id LIMIT ?",
			documents(scope)
		);
		let rows = sqlx::query(&sql)
			.bind(after.0 as i64)
			.bind(after.1)
			.bind(limit as i64)
			.fetch_all(&self.pool)
			.await?;
		Ok(rows
			.into_iter()
			.filter_map(|row| {
				Some(PendingVectors {
					origin: Origin::from_i64(row.get(0))?,
					doc_id: row.get(1),
					stamp: row.get(2),
					kind: row.get(3),
				})
			})
			.collect())
	}

	/// How many documents [`Self::pending`] would hand out from the start.
	pub async fn pending_count(&self, scope: &DeepScope) -> Result<u64> {
		let sql = format!(
			"SELECT COUNT(*) FROM ({}) v WHERE {STALE}",
			documents(scope)
		);
		let count: i64 = sqlx::query_scalar(&sql).fetch_one(&self.pool).await?;
		Ok(count.max(0) as u64)
	}

	/// A document's chunks, read from `text.db` as it is now.
	pub async fn chunks(&self, doc: &PendingVectors) -> Result<Vec<Chunk>> {
		match doc.origin {
			Origin::Text => {
				let body: Option<String> =
					sqlx::query_scalar("SELECT body FROM txt.text_fts WHERE rowid = ?")
						.bind(doc.doc_id)
						.fetch_optional(&self.pool)
						.await?;
				Ok(body.map_or_else(Vec::new, |body| {
					chunk_text(&body, CHUNK_CHARS, CHUNK_OVERLAP, MAX_CHUNKS_PER_DOC)
				}))
			}
			Origin::Transcript => Ok(chunk_spans(
				&self.spans("transcript", doc.doc_id).await?,
				CHUNK_CHARS,
				MAX_CHUNKS_PER_DOC,
			)),
			// A described moment is short and its time matters, so each
			// stays its own chunk.
			Origin::Description => Ok(chunk_spans(
				&self.spans("description", doc.doc_id).await?,
				1,
				MAX_CHUNKS_PER_DOC,
			)),
		}
	}

	async fn spans(&self, table: &str, doc_id: i64) -> Result<Vec<Span>> {
		let sql = format!(
			"SELECT s.id, s.start_ms, s.end_ms, t.body FROM txt.{table}_span s \
			 JOIN txt.{table}_fts t ON t.rowid = s.id WHERE s.doc_id = ? ORDER BY s.id"
		);
		Ok(sqlx::query(&sql)
			.bind(doc_id)
			.fetch_all(&self.pool)
			.await?
			.into_iter()
			.map(|row| Span {
				id: row.get(0),
				start_ms: row.get(1),
				end_ms: row.get(2),
				text: row.get(3),
			})
			.collect())
	}

	/// Store a document's vectors, one per chunk, replacing what it had.
	/// A document with no chunks is recorded too, so it leaves the queue.
	pub async fn write(
		&self,
		doc: &PendingVectors,
		chunks: &[Chunk],
		vectors: &[Vec<f32>],
	) -> Result<()> {
		if chunks.len() != vectors.len() {
			return Err(Error::Other(format!(
				"{} chunks but {} vectors",
				chunks.len(),
				vectors.len()
			)));
		}
		let _writer = self.writer.lock().await;
		let mut tx = self.pool.begin().await?;
		sqlx::query("DELETE FROM vec_chunk WHERE origin = ? AND doc_id = ?")
			.bind(doc.origin as i64)
			.bind(doc.doc_id)
			.execute(&mut *tx)
			.await?;
		for (chunk, vector) in chunks.iter().zip(vectors) {
			let (scale, bytes) = quantize(vector);
			sqlx::query(
				"INSERT INTO vec_chunk (origin, doc_id, a, b, start_ms, end_ms, scale, vector) \
				 VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
			)
			.bind(doc.origin as i64)
			.bind(doc.doc_id)
			.bind(chunk.a)
			.bind(chunk.b)
			.bind(chunk.start_ms)
			.bind(chunk.end_ms)
			.bind(scale)
			.bind(bytes)
			.execute(&mut *tx)
			.await?;
		}
		sqlx::query(
			"INSERT INTO vec_doc (origin, doc_id, stamp) VALUES (?, ?, ?) \
			 ON CONFLICT (origin, doc_id) DO UPDATE SET stamp = excluded.stamp",
		)
		.bind(doc.origin as i64)
		.bind(doc.doc_id)
		.bind(&doc.stamp)
		.execute(&mut *tx)
		.await?;
		tx.commit().await?;
		self.generation.fetch_add(1, AtomicOrdering::SeqCst);
		Ok(())
	}

	/// Remove vectors whose document is gone, was rewritten, or left
	/// `scope`. Returns how many documents lost their vectors.
	pub async fn prune(&self, scope: &DeepScope) -> Result<u64> {
		let _writer = self.writer.lock().await;
		let mut conn = self.pool.acquire().await?;
		let mut tx = conn.begin().await?;
		sqlx::query(
			"CREATE TEMP TABLE IF NOT EXISTS vec_valid \
			 (origin INTEGER, doc_id INTEGER, stamp TEXT, PRIMARY KEY (origin, doc_id))",
		)
		.execute(&mut *tx)
		.await?;
		sqlx::query("DELETE FROM temp.vec_valid")
			.execute(&mut *tx)
			.await?;
		sqlx::query(&format!(
			"INSERT OR IGNORE INTO temp.vec_valid SELECT origin, doc_id, stamp FROM ({})",
			documents(scope)
		))
		.execute(&mut *tx)
		.await?;
		let pruned = sqlx::query(
			"DELETE FROM vec_doc WHERE NOT EXISTS (SELECT 1 FROM temp.vec_valid v \
			 WHERE v.origin = vec_doc.origin AND v.doc_id = vec_doc.doc_id AND v.stamp = vec_doc.stamp)",
		)
		.execute(&mut *tx)
		.await?
		.rows_affected();
		sqlx::query(
			"DELETE FROM vec_chunk WHERE NOT EXISTS (SELECT 1 FROM vec_doc d \
			 WHERE d.origin = vec_chunk.origin AND d.doc_id = vec_chunk.doc_id)",
		)
		.execute(&mut *tx)
		.await?;
		sqlx::query("DELETE FROM temp.vec_valid")
			.execute(&mut *tx)
			.await?;
		tx.commit().await?;
		if pruned > 0 {
			self.generation.fetch_add(1, AtomicOrdering::SeqCst);
		}
		Ok(pruned)
	}

	/// Drop every vector and give the space back.
	pub async fn clear(&self) -> Result<()> {
		let _writer = self.writer.lock().await;
		let mut conn = self.pool.acquire().await?;
		for statement in [
			"DELETE FROM vec_chunk",
			"DELETE FROM vec_doc",
			"VACUUM",
			"PRAGMA wal_checkpoint(TRUNCATE)",
		] {
			sqlx::query(statement).execute(&mut *conn).await?;
		}
		self.generation.fetch_add(1, AtomicOrdering::SeqCst);
		Ok(())
	}

	pub async fn counts(&self) -> Result<VectorIndexCounts> {
		let (documents, chunks): (i64, i64) = sqlx::query_as(
			"SELECT (SELECT COUNT(*) FROM vec_doc), (SELECT COUNT(*) FROM vec_chunk)",
		)
		.fetch_one(&self.pool)
		.await?;
		let model: Option<String> =
			sqlx::query_scalar("SELECT value FROM vec_meta WHERE key = 'model'")
				.fetch_optional(&self.pool)
				.await?;
		Ok(VectorIndexCounts {
			documents: documents.max(0) as u64,
			chunks: chunks.max(0) as u64,
			model: model.map(|m| {
				m.rsplit_once(':')
					.map_or(m.clone(), |(id, _)| id.to_string())
			}),
		})
	}

	/// Files ranked by their best chunk's similarity to `search.vector`,
	/// best first. A chunk counts only while its document's row is the one
	/// it was made from and its file sits in the scope.
	pub async fn search(&self, search: &VectorSearch) -> Result<Vec<VectorHit>> {
		if search.limit == 0 || search.scope.is_empty() {
			return Ok(Vec::new());
		}
		let matrix = self.matrix().await?;
		if matrix.ids.is_empty() {
			return Ok(Vec::new());
		}
		if matrix.dim != search.vector.len() {
			return Err(Error::Other(format!(
				"the query has {} dimensions and the index {}",
				search.vector.len(),
				matrix.dim
			)));
		}
		// Eligibility comes before ranking, so chunks outside the folder, the
		// scope or the kinds never crowd out the ones that can answer.
		let mut eligible: HashMap<i64, (i64, i64)> = HashMap::new();
		let (sql, binds) = selection(search, None, false);
		let mut query = sqlx::query(&sql);
		for bind in binds {
			query = query.bind(bind);
		}
		for row in query.fetch_all(&self.pool).await? {
			eligible.insert(row.get(0), (row.get(1), row.get(2)));
		}
		// One entry per document, its best chunk, so a long document counts
		// once however many of its chunks lead.
		let mut best: HashMap<(i64, i64), Scored> = HashMap::new();
		for (row, id) in matrix.ids.iter().enumerate() {
			let Some(doc) = eligible.get(id) else {
				continue;
			};
			let bytes = &matrix.data[row * matrix.dim..(row + 1) * matrix.dim];
			let score = dot_quantized(&search.vector, matrix.scales[row], bytes);
			let entry = best.entry(*doc).or_insert(Scored(f32::MIN, *id));
			if score > entry.0 {
				*entry = Scored(score, *id);
			}
		}
		let mut ranked: Vec<Scored> = best.into_values().collect();
		// Ties fall back to the chunk id so every page of a query sees one order.
		ranked.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));

		// A document answers for its file, or for every file with its bytes,
		// so resolve the leaders a page at a time until enough files are found.
		let mut out: Vec<VectorHit> = Vec::new();
		let mut seen = std::collections::HashSet::new();
		for page in ranked.chunks(search.limit.max(16)) {
			let similarity: HashMap<i64, f32> = page.iter().map(|s| (s.1, s.0)).collect();
			let ids: Vec<String> = page.iter().map(|s| s.1.to_string()).collect();
			let (sql, binds) = selection(search, Some(&ids.join(",")), true);
			let mut query = sqlx::query(&sql);
			for bind in binds {
				query = query.bind(bind);
			}
			let mut hits: Vec<(i64, VectorHit)> = query
				.fetch_all(&self.pool)
				.await?
				.into_iter()
				.filter_map(|row| {
					let chunk: i64 = row.get(0);
					let text: Option<String> = row.get(6);
					Some((
						chunk,
						VectorHit {
							record_uuid: row.get(1),
							external_id: address(row.get(2), row.get(3), row.get(4))?,
							kind: row.get(5),
							similarity: similarity.get(&chunk).copied()?,
							text: text.unwrap_or_default().trim().to_string(),
							start_ms: row.get(7),
							end_ms: row.get(8),
						},
					))
				})
				.collect();
			// The same order as the ranking, so page boundaries never reorder ties.
			hits.sort_by(|(a_chunk, a), (b_chunk, b)| {
				b.similarity
					.total_cmp(&a.similarity)
					.then(a_chunk.cmp(b_chunk))
					.then(a.record_uuid.cmp(&b.record_uuid))
			});
			for (_, hit) in hits {
				if seen.insert(hit.record_uuid) {
					out.push(hit);
					if out.len() == search.limit {
						return Ok(out);
					}
				}
			}
		}
		Ok(out)
	}

	async fn matrix(&self) -> Result<Arc<Matrix>> {
		let generation = self.generation.load(AtomicOrdering::SeqCst);
		let mut cached = self.matrix.lock().await;
		if let Some(matrix) = cached.as_ref().filter(|m| m.generation == generation) {
			return Ok(matrix.clone());
		}
		let rows = sqlx::query("SELECT id, scale, vector FROM vec_chunk ORDER BY id")
			.fetch_all(&self.pool)
			.await?;
		let dim = rows.first().map_or(0, |row| row.get::<Vec<u8>, _>(2).len());
		let mut matrix = Matrix {
			generation,
			dim,
			ids: Vec::with_capacity(rows.len()),
			scales: Vec::with_capacity(rows.len()),
			data: Vec::with_capacity(rows.len() * dim),
		};
		for row in rows {
			let bytes: Vec<u8> = row.get(2);
			if bytes.len() != dim {
				continue;
			}
			matrix.ids.push(row.get(0));
			matrix.scales.push(row.get::<f64, _>(1) as f32);
			matrix.data.extend(bytes.into_iter().map(|b| b as i8));
		}
		let matrix = Arc::new(matrix);
		*cached = Some(matrix.clone());
		Ok(matrix)
	}
}

/// A document's best chunk: its similarity and its id.
struct Scored(f32, i64);

/// Which write of `text.db` a document row came from: its id and the
/// revision every write raises. Row ids are reused after a delete, so the id
/// alone would mistake a rewritten caption for the one it replaced.
const TEXT_STAMP: &str = "(d.id || ':' || d.rev)";
const TRANSCRIPT_STAMP: &str = TEXT_STAMP;
const DESCRIPTION_STAMP: &str = TEXT_STAMP;

/// Holds for a document `v` whose vectors are missing or stale.
const STALE: &str = "NOT EXISTS (SELECT 1 FROM vec_doc e \
	WHERE e.origin = v.origin AND e.doc_id = v.doc_id AND e.stamp = v.stamp)";

/// Every document in `scope` that has text to embed, with its stamp:
/// `(origin, doc_id, stamp, kind)`.
fn documents(scope: &DeepScope) -> String {
	let scope = scope.condition("r.parent_uuid");
	let current = "JOIN src.record r ON r.uuid = d.record_uuid AND r.type = 'file' \
		JOIN src.facet_file f ON f.record_uuid = r.uuid \
		WHERE d.skipped IS NULL AND f.size IS d.size AND f.mtime IS d.mtime \
		AND lower(f.extension) = d.extension";
	format!(
		"SELECT 0 AS origin, d.id AS doc_id, {TEXT_STAMP} AS stamp, d.kind AS kind \
		 FROM txt.text_doc d {current} AND d.text_bytes > 0 AND {scope} \
		 UNION ALL \
		 SELECT 1, d.id, {TRANSCRIPT_STAMP}, '{TRANSCRIPT_KIND}' \
		 FROM txt.transcript_doc d {current} AND {scope} \
		 AND EXISTS (SELECT 1 FROM txt.transcript_span s WHERE s.doc_id = d.id) \
		 UNION ALL \
		 SELECT 2, d.id, {DESCRIPTION_STAMP}, d.kind FROM txt.description_doc d \
		 WHERE EXISTS (SELECT 1 FROM src.content k \
		 JOIN src.record r ON r.content_id = k.id AND r.type = 'file' \
		 JOIN src.facet_file f ON f.record_uuid = r.uuid AND f.link_target IS NULL \
		 WHERE (k.uuid = d.content_uuid OR k.candidate_uuid = d.content_uuid) AND {scope})"
	)
}

/// The chunks a search may return, with their binds: fresh (the document
/// row is the one the vectors came from and the file has not changed since
/// its text was read), inside the deep scope, under the searched path and of
/// a wanted kind. With `ids`, only those chunks. Without `with_text` each
/// row is `(chunk, origin, doc)`; with it, the hit's record, path, kind, the
/// chunk's text and its times.
fn selection(search: &VectorSearch, ids: Option<&str>, with_text: bool) -> (String, Vec<String>) {
	let scope = search.scope.condition("r.parent_uuid");
	let under = search.under.trim_matches('/');
	let mut filter = String::new();
	let mut binds = Vec::new();
	if let Some(ids) = ids {
		filter.push_str(&format!(" AND c.id IN ({ids})"));
	}
	if !under.is_empty() {
		filter.push_str(&format!(
			" AND ({RECORD_PATH} = ? OR substr({RECORD_PATH}, 1, length(?)) = ?)"
		));
	}
	let kinds = |kind: &str| {
		if search.kinds.is_empty() {
			return String::new();
		}
		let quoted: Vec<String> = search
			.kinds
			.iter()
			.map(|k| format!("'{}'", k.replace('\'', "''")))
			.collect();
		format!(" AND {kind} IN ({})", quoted.join(", "))
	};
	let columns = |kind: &str, text: &str, times: &str| {
		if with_text {
			format!("c.id, r.uuid, r.parent_uuid, p.path, r.title, {kind}, {text}, {times}")
		} else {
			"c.id, c.origin, c.doc_id".to_string()
		}
	};
	let chunk = "FROM vec_chunk c JOIN vec_doc e ON e.origin = c.origin AND e.doc_id = c.doc_id";
	let file = "JOIN src.record r ON r.uuid = d.record_uuid AND r.type = 'file' \
		JOIN src.facet_file f ON f.record_uuid = r.uuid \
		LEFT JOIN src.directory_path p ON p.record_uuid = r.parent_uuid";
	let current = "f.size IS d.size AND f.mtime IS d.mtime AND lower(f.extension) = d.extension";
	let transcript_kind = format!("'{TRANSCRIPT_KIND}'");
	let sql = format!(
		"SELECT {} {chunk} JOIN txt.text_doc d ON d.id = c.doc_id {file} \
		 WHERE c.origin = 0 AND e.stamp = {TEXT_STAMP} AND {current} AND {scope}{filter}{} \
		 UNION ALL \
		 SELECT {} {chunk} JOIN txt.transcript_doc d ON d.id = c.doc_id {file} \
		 WHERE c.origin = 1 AND e.stamp = {TRANSCRIPT_STAMP} AND {current} AND {scope}{filter}{} \
		 UNION ALL \
		 SELECT {} {chunk} JOIN txt.description_doc d ON d.id = c.doc_id \
		 JOIN src.content k ON k.uuid = d.content_uuid OR k.candidate_uuid = d.content_uuid \
		 JOIN src.record r ON r.content_id = k.id AND r.type = 'file' \
		 JOIN src.facet_file f ON f.record_uuid = r.uuid AND f.link_target IS NULL \
		 LEFT JOIN src.directory_path p ON p.record_uuid = r.parent_uuid \
		 WHERE c.origin = 2 AND e.stamp = {DESCRIPTION_STAMP} AND {scope}{filter}{}",
		columns(
			"d.kind",
			"(SELECT substr(body, c.a + 1, c.b) FROM txt.text_fts WHERE rowid = d.id)",
			"NULL, NULL"
		),
		kinds("d.kind"),
		columns(
			&transcript_kind,
			"(SELECT group_concat(t.body, ' ') FROM txt.transcript_span s \
			 JOIN txt.transcript_fts t ON t.rowid = s.id WHERE s.doc_id = d.id AND s.id BETWEEN c.a AND c.b)",
			"c.start_ms, c.end_ms"
		),
		kinds(&transcript_kind),
		columns(
			"d.kind",
			"(SELECT t.body FROM txt.description_fts t WHERE t.rowid = c.a)",
			"c.start_ms, c.end_ms"
		),
		kinds("d.kind"),
	);
	if !under.is_empty() {
		for _ in 0..3 {
			binds.extend([under.to_string(), format!("{under}/"), format!("{under}/")]);
		}
	}
	(sql, binds)
}

/// One file a hybrid search found.
#[derive(Debug, Clone)]
pub struct HybridHit {
	pub record_uuid: Uuid,
	/// Relative to the source root.
	pub external_id: String,
	pub kind: String,
	/// The fused score. Larger is better; only the order means anything.
	pub score: f64,
	/// The file's place in the word ranking, from 1, when its words matched.
	pub lexical_rank: Option<usize>,
	/// Cosine similarity of its best chunk, when it has vectors.
	pub similarity: Option<f32>,
	/// The word match's snippet when there is one, otherwise the best
	/// chunk's text.
	pub snippet: Vec<SnippetPart>,
	pub start_ms: Option<i64>,
	pub end_ms: Option<i64>,
}

/// A search that ranks by words and by meaning at once.
#[derive(Debug, Clone, Default)]
pub struct HybridSearch {
	/// What the person typed.
	pub query: String,
	/// Its embedding, from the embedder the index was written with.
	pub vector: Vec<f32>,
	pub under: String,
	pub scope: DeepScope,
	pub kinds: Vec<String>,
	pub limit: usize,
	pub offset: usize,
	pub snippet_tokens: u32,
}

/// Rank files by words and by meaning, fused by reciprocal rank.
///
/// The word side matches any of the query's words rather than all of them,
/// because a question rarely repeats a document's exact words; BM25 still
/// ranks files with more of them first. A file found by both rankings
/// outranks one found by either alone. A file with no vectors yet, because
/// the embedding pass has not reached it, is still found by its words.
pub async fn hybrid_search(
	text: &ContentTextIndex,
	vectors: &VectorIndex,
	search: &HybridSearch,
) -> Result<Vec<HybridHit>> {
	let depth = (search.offset + search.limit).clamp(1, 1000) * 2;
	let expression = any_word_expression(&search.query);
	let lexical = if expression.is_empty() {
		Vec::new()
	} else {
		text.search(&TextSearch {
			expression,
			under: search.under.clone(),
			kinds: search.kinds.clone(),
			scope: Some(search.scope.clone()),
			limit: depth * 2,
			offset: 0,
			snippet_tokens: search.snippet_tokens,
		})
		.await?
	};
	let semantic = vectors
		.search(&VectorSearch {
			vector: search.vector.clone(),
			under: search.under.clone(),
			scope: search.scope.clone(),
			kinds: search.kinds.clone(),
			limit: depth,
		})
		.await?;

	let mut lexical_by_file = Vec::new();
	let mut seen = std::collections::HashSet::new();
	for hit in lexical {
		if seen.insert(hit.record_uuid) {
			lexical_by_file.push(hit);
		}
	}
	let fused = fuse(
		&lexical_by_file
			.iter()
			.map(|h| h.record_uuid)
			.collect::<Vec<_>>(),
		&semantic.iter().map(|h| h.record_uuid).collect::<Vec<_>>(),
	);
	let lexical_at: HashMap<Uuid, usize> = lexical_by_file
		.iter()
		.enumerate()
		.map(|(i, h)| (h.record_uuid, i))
		.collect();
	let semantic_at: HashMap<Uuid, usize> = semantic
		.iter()
		.enumerate()
		.map(|(i, h)| (h.record_uuid, i))
		.collect();

	Ok(fused
		.into_iter()
		.skip(search.offset)
		.take(search.limit)
		.filter_map(|(uuid, score)| {
			let word = lexical_at.get(&uuid).map(|&i| &lexical_by_file[i]);
			let meaning = semantic_at.get(&uuid).map(|&i| &semantic[i]);
			let (external_id, kind) = match (word, meaning) {
				(Some(w), _) => (w.external_id.clone(), w.kind.clone()),
				(None, Some(m)) => (m.external_id.clone(), m.kind.clone()),
				(None, None) => return None,
			};
			let (snippet, start_ms, end_ms) = match (word, meaning) {
				(Some(w), _) => (w.snippet.clone(), w.start_ms, w.end_ms),
				(None, Some(m)) => (
					vec![SnippetPart {
						text: excerpt(&m.text, 200),
						matched: false,
					}],
					m.start_ms,
					m.end_ms,
				),
				(None, None) => unreachable!(),
			};
			Some(HybridHit {
				record_uuid: uuid,
				external_id,
				kind,
				score,
				lexical_rank: lexical_at.get(&uuid).map(|i| i + 1),
				similarity: meaning.map(|m| m.similarity),
				snippet,
				start_ms,
				end_ms,
			})
		})
		.collect())
}

/// Reciprocal rank fusion of two rankings of files, best first. Ties keep
/// the semantic order, then the lexical order.
pub fn fuse(lexical: &[Uuid], semantic: &[Uuid]) -> Vec<(Uuid, f64)> {
	let mut scores: HashMap<Uuid, (f64, usize)> = HashMap::new();
	for (rank, uuid) in semantic.iter().enumerate() {
		let entry = scores.entry(*uuid).or_insert((0.0, rank));
		entry.0 += 1.0 / (FUSION_K + rank as f64 + 1.0);
	}
	for (rank, uuid) in lexical.iter().enumerate() {
		let entry = scores.entry(*uuid).or_insert((0.0, semantic.len() + rank));
		entry.0 += 1.0 / (FUSION_K + rank as f64 + 1.0);
	}
	let mut fused: Vec<(Uuid, f64, usize)> = scores
		.into_iter()
		.map(|(uuid, (score, order))| (uuid, score, order))
		.collect();
	fused.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.2.cmp(&b.2)));
	fused
		.into_iter()
		.map(|(uuid, score, _)| (uuid, score))
		.collect()
}

/// The first `max` characters of `text`, cut at a word.
fn excerpt(text: &str, max: usize) -> String {
	if text.chars().count() <= max {
		return text.to_string();
	}
	let cut: String = text.chars().take(max).collect();
	let cut = cut
		.rsplit_once(char::is_whitespace)
		.map_or(cut.as_str(), |(head, _)| head);
	format!("{cut}…")
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn text_is_cut_at_the_strongest_break_and_overlaps() {
		let para = "One sentence here. Another one follows.";
		let text = format!("{para}\n\n{para}\n\n{para}");
		let chunks = chunk_text(&text, 50, 10, 10);
		assert!(chunks.len() >= 3);
		assert_eq!(chunks[0].text, para);
		for chunk in &chunks {
			let slice: String = text
				.chars()
				.skip(chunk.a as usize)
				.take(chunk.b as usize)
				.collect();
			assert_eq!(slice.trim(), chunk.text, "offsets point at the chunk");
			assert!(chunk.text.chars().count() <= 50);
		}
		let long = "word ".repeat(100);
		let chunks = chunk_text(&long, 40, 10, 100);
		for pair in chunks.windows(2) {
			let end = pair[0].a + pair[0].b;
			assert!(pair[1].a < end, "neighbours overlap");
			assert!(pair[1].a > pair[0].a, "chunks move forward");
		}
		assert!(chunk_text("   \n  ", 10, 2, 5).is_empty());
		assert_eq!(chunk_text(&long, 40, 10, 3).len(), 3);
		let wide = "日本語の文章です。".repeat(30);
		assert!(chunk_text(&wide, 20, 5, 100)
			.iter()
			.all(|c| c.text.chars().count() <= 20));
	}

	#[test]
	fn spans_group_into_timed_chunks_without_splitting() {
		let span = |id, start, text: &str| Span {
			id,
			start_ms: Some(start),
			end_ms: Some(start + 900),
			text: text.to_string(),
		};
		let spans = [
			span(1, 0, "hello there"),
			span(2, 1000, "general kenobi"),
			span(3, 2000, "  "),
			span(4, 3000, "you are a bold one"),
		];
		let chunks = chunk_spans(&spans, 30, 10);
		assert_eq!(chunks.len(), 2);
		assert_eq!(chunks[0].text, "hello there general kenobi");
		assert_eq!((chunks[0].a, chunks[0].b), (1, 2));
		assert_eq!(
			(chunks[0].start_ms, chunks[0].end_ms),
			(Some(0), Some(1900))
		);
		assert_eq!((chunks[1].a, chunks[1].b), (4, 4));
		assert_eq!(chunk_spans(&spans, 1, 10).len(), 3, "one per span");
		assert_eq!(chunk_spans(&spans, 1, 2).len(), 2);
	}

	#[test]
	fn quantized_vectors_keep_the_cosine() {
		let a = [0.6f32, 0.8, 0.0];
		let b = [0.8f32, 0.6, 0.0];
		let (scale, bytes) = quantize(&b);
		let bytes: Vec<i8> = bytes.into_iter().map(|b| b as i8).collect();
		let exact: f32 = a.iter().zip(&b).map(|(x, y)| x * y).sum();
		assert!((dot_quantized(&a, scale, &bytes) - exact).abs() < 0.01);
		assert_eq!(quantize(&[0.0, 0.0]).0, 0.0);
	}

	#[test]
	fn a_file_both_rankings_find_leads() {
		let [a, b, c, d] = [
			Uuid::now_v7(),
			Uuid::now_v7(),
			Uuid::now_v7(),
			Uuid::now_v7(),
		];
		let fused = fuse(&[a, b, c], &[d, c, a]);
		let order: Vec<Uuid> = fused.iter().map(|(u, _)| *u).collect();
		assert_eq!(order[0], a, "first by words and third by meaning");
		assert_eq!(order[1], c);
		assert!(order.contains(&b) && order.contains(&d));
		assert_eq!(order.len(), 4);
		let only_meaning = fuse(&[], &[d, a]);
		assert_eq!(only_meaning[0].0, d);
	}
}
