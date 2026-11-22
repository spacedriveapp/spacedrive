//! # Content text index
//!
//! A full-text index over what a source's files say, kept beside the source
//! store in its own SQLite file, `text.db`. It belongs to the store's
//! generation half: every row is extracted from bytes the origin still holds,
//! so nothing here is the only copy of anything, and it never holds an
//! assertion. Dropping it loses nothing a pass over the origin cannot
//! produce again.
//!
//! It is a separate file rather than tables in `data.db` for two reasons.
//! SQLite allows one writer per file, and extracted text is large, so
//! writing it into the record store would hold the lock the walk and the
//! watcher commit through. And a replica of the record store is a `VACUUM
//! INTO` copy, which should not carry a megabyte of text per file to a
//! device that can extract its own.
//!
//! Each connection attaches the record store as `src`, so the work queue
//! and the search are joins rather than two lookups stitched together in
//! code. A row is keyed by record uuid and stamped with the size, mtime and
//! extension the record had when its text was read. A record whose facet no longer
//! matches its row is pending again, and a search only returns rows whose
//! stamp still matches, so an edited file drops out of results until its new
//! text lands rather than answering with what it used to say. A rename keeps
//! the record uuid, so it costs nothing: paths are read from the record
//! store at query time. A deleted record's row is unreachable from a search
//! at once and removed by the next [`ContentTextIndex::prune`].

use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions};
use sqlx::{Connection, Row};
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::file::address;

/// The sidecar's file name inside a source's directory.
pub const TEXT_INDEX_FILE: &str = "text.db";

/// Highlight delimiters handed to FTS5 `snippet()`. Private-use code points
/// cannot occur in text the tokenizer matched, so splitting on them is exact.
const MATCH_START: char = '\u{E000}';
const MATCH_END: char = '\u{E001}';

const TEXT_SCHEMA: [&str; 3] = [
	"CREATE TABLE IF NOT EXISTS text_doc (
    id INTEGER PRIMARY KEY,
    record_uuid BLOB NOT NULL UNIQUE,
    size INTEGER,
    mtime INTEGER,
    extension TEXT NOT NULL,
    kind TEXT NOT NULL,
    text_bytes INTEGER NOT NULL,
    truncated INTEGER NOT NULL DEFAULT 0,
    skipped TEXT
)",
	"CREATE VIRTUAL TABLE IF NOT EXISTS text_fts USING fts5(
    body,
    tokenize = 'unicode61 remove_diacritics 2'
)",
	"CREATE TABLE IF NOT EXISTS text_meta (key TEXT PRIMARY KEY, value INTEGER NOT NULL)",
];

/// Spoken words, one FTS row per transcript segment so a hit carries the
/// time it was said. Kept apart from `text_doc` because no extractor can
/// rebuild it cheaply: `clear` leaves it, and the transcription job that
/// wrote it is what refills it, from the transcript sidecar when one exists.
const TRANSCRIPT_SCHEMA: [&str; 4] = [
	"CREATE TABLE IF NOT EXISTS transcript_doc (
    id INTEGER PRIMARY KEY,
    record_uuid BLOB NOT NULL UNIQUE,
    size INTEGER,
    mtime INTEGER,
    extension TEXT NOT NULL,
    language TEXT,
    model TEXT NOT NULL,
    duration_ms INTEGER,
    skipped TEXT
)",
	"CREATE TABLE IF NOT EXISTS transcript_span (
    id INTEGER PRIMARY KEY,
    doc_id INTEGER NOT NULL,
    start_ms INTEGER NOT NULL,
    end_ms INTEGER NOT NULL
)",
	"CREATE INDEX IF NOT EXISTS transcript_span_doc ON transcript_span (doc_id)",
	"CREATE VIRTUAL TABLE IF NOT EXISTS transcript_fts USING fts5(
    body,
    tokenize = 'unicode61 remove_diacritics 2'
)",
];

/// The kind a transcript hit reports, beside the extractor names.
pub const TRANSCRIPT_KIND: &str = "transcript";

/// A file whose text is missing from the index or was read from other bytes.
#[derive(Debug, Clone)]
pub struct PendingText {
	pub record_uuid: Uuid,
	/// Row id in the record store, the cursor a pass claims by.
	pub rowid: i64,
	/// Path relative to the source root.
	pub external_id: String,
	pub size: Option<i64>,
	pub mtime: Option<i64>,
	/// Lowercase, without the dot.
	pub extension: String,
}

/// What a pass read from one file.
#[derive(Debug, Clone)]
pub struct TextDoc {
	pub record_uuid: Uuid,
	/// The stamp of the bytes the text came from, copied from [`PendingText`].
	pub size: Option<i64>,
	pub mtime: Option<i64>,
	/// The extension the file had when it was read, lowercase. A rename to
	/// another extension reads it again, since another extractor may apply.
	pub extension: String,
	/// The extractor that read it, such as `markdown` or `pdf`.
	pub kind: String,
	pub text: String,
	/// The file held more text than the cap and only the start was indexed.
	pub truncated: bool,
	/// Why the file yielded no text, when it was read and nothing was
	/// indexable. The row still records the stamp so the bytes are not read
	/// again until they change.
	pub skipped: Option<String>,
}

/// An audio or video file whose transcript is missing or was made from
/// other bytes.
#[derive(Debug, Clone)]
pub struct PendingTranscript {
	pub record_uuid: Uuid,
	/// Row id in the record store, the cursor a pass claims by.
	pub rowid: i64,
	/// Path relative to the source root.
	pub external_id: String,
	pub size: Option<i64>,
	pub mtime: Option<i64>,
	/// Lowercase, without the dot.
	pub extension: String,
	/// The candidate content uuid, or the content uuid when the record has
	/// no candidate. None until identification reaches the file.
	pub content_uuid: Option<Uuid>,
}

/// One timed piece of a transcript.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptSpan {
	pub start_ms: i64,
	pub end_ms: i64,
	pub text: String,
}

/// What a transcription pass made of one file.
#[derive(Debug, Clone)]
pub struct TranscriptDoc {
	pub record_uuid: Uuid,
	/// The stamp of the bytes the transcript came from.
	pub size: Option<i64>,
	pub mtime: Option<i64>,
	pub extension: String,
	/// Detected or requested language, such as `en`.
	pub language: Option<String>,
	/// The model that made it, such as `whisper-base`.
	pub model: String,
	pub duration_ms: Option<i64>,
	pub spans: Vec<TranscriptSpan>,
	/// Why the file has no transcript, such as `no audio track`. The row
	/// still records the stamp so the file is not tried again until it
	/// changes.
	pub skipped: Option<String>,
}

/// A search over the index.
#[derive(Debug, Clone, Default)]
pub struct TextSearch {
	/// An FTS5 expression. Build it with [`match_expression`].
	pub expression: String,
	/// Only files at or under this path, relative to the source root. Empty
	/// means the whole source.
	pub under: String,
	/// Only files read by these extractors. Empty means every kind.
	pub kinds: Vec<String>,
	pub limit: usize,
	pub offset: usize,
	/// Tokens of context the snippet keeps around the best match.
	pub snippet_tokens: u32,
}

/// A piece of a snippet, either matched text or the text around it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnippetPart {
	pub text: String,
	pub matched: bool,
}

/// One ranked hit.
#[derive(Debug, Clone)]
pub struct TextHit {
	pub record_uuid: Uuid,
	/// Relative to the source root.
	pub external_id: String,
	pub kind: String,
	/// BM25, negated so a larger number is a better match.
	pub score: f64,
	pub snippet: Vec<SnippetPart>,
	pub truncated: bool,
	/// Where in the recording the match was said, for a transcript hit.
	pub start_ms: Option<i64>,
	pub end_ms: Option<i64>,
}

/// What the index holds.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TextIndexCounts {
	/// Files with a row, text or not.
	pub files: u64,
	/// Of those, files whose text was cut at the cap.
	pub truncated: u64,
	/// Of those, files read with nothing to index.
	pub skipped: u64,
}

/// The content text index of one source.
#[derive(Debug, Clone)]
pub struct ContentTextIndex {
	pool: SqlitePool,
	path: PathBuf,
	/// Held across every write transaction. Two passes over one source can
	/// run at once (a rebuild beside an ordinary pass), and a deferred
	/// transaction that reads before it writes fails outright when another
	/// commits in between, so writes take turns instead.
	writer: std::sync::Arc<tokio::sync::Mutex<()>>,
}

impl ContentTextIndex {
	/// Open the index in `source_dir`, creating it on first use, with the
	/// record store at `source_dir/data.db` attached for reads.
	pub async fn open(source_dir: &Path) -> Result<Self> {
		let path = source_dir.join(TEXT_INDEX_FILE);
		let store = source_dir.join("data.db");
		if !store.exists() {
			return Err(Error::SourceNotFound(store.display().to_string()));
		}
		let store = store
			.to_str()
			.ok_or_else(|| Error::Other(format!("store path is not UTF-8: {}", store.display())))?
			.to_string();

		let options = SqliteConnectOptions::from_str(&format!("sqlite:{}", path.display()))
			.map_err(|e| Error::Other(format!("invalid database path: {e}")))?
			.create_if_missing(true)
			.journal_mode(SqliteJournalMode::Wal)
			.busy_timeout(Duration::from_secs(5));
		let pool = SqlitePoolOptions::new()
			.max_connections(4)
			.after_release(|conn, _meta| Box::pin(async move { Ok(!conn.is_in_transaction()) }))
			.after_connect(move |conn, _meta| {
				let store = store.clone();
				Box::pin(async move {
					sqlx::query("ATTACH DATABASE ? AS src")
						.bind(store)
						.execute(&mut *conn)
						.await?;
					Ok(())
				})
			})
			.connect_with(options)
			.await?;

		let mut tx = pool.begin().await?;
		for statement in TEXT_SCHEMA.into_iter().chain(TRANSCRIPT_SCHEMA) {
			sqlx::query(statement).execute(&mut *tx).await?;
		}
		tx.commit().await?;
		Ok(Self {
			pool,
			path,
			writer: Default::default(),
		})
	}

	/// The sidecar's file.
	pub fn path(&self) -> &Path {
		&self.path
	}

	pub fn pool(&self) -> &SqlitePool {
		&self.pool
	}

	/// Close every connection. The file stays.
	pub async fn close(&self) {
		self.pool.close().await;
	}

	/// File records past `after` with one of `extensions` whose row is
	/// missing or stamped with other bytes, in row order.
	pub async fn pending(
		&self,
		extensions: &[&str],
		after: i64,
		limit: usize,
	) -> Result<Vec<PendingText>> {
		if extensions.is_empty() {
			return Ok(Vec::new());
		}
		let sql = format!(
			"SELECT r.rowid, r.uuid, r.parent_uuid, p.path, r.title, f.size, f.mtime, \
			 lower(f.extension) {} AND r.rowid > ? ORDER BY r.rowid LIMIT ?",
			pending_clause(extensions.len())
		);
		let mut query = sqlx::query(&sql);
		for extension in extensions {
			query = query.bind(extension.to_string());
		}
		let rows = query
			.bind(after)
			.bind(limit as i64)
			.fetch_all(&self.pool)
			.await?;

		Ok(rows
			.into_iter()
			.filter_map(|row| {
				Some(PendingText {
					rowid: row.get(0),
					record_uuid: row.get(1),
					external_id: address(row.get(2), row.get(3), row.get(4))?,
					size: row.get(5),
					mtime: row.get(6),
					extension: row.get(7),
				})
			})
			.collect())
	}

	/// How many files [`Self::pending`] would hand out from the start.
	pub async fn pending_count(&self, extensions: &[&str]) -> Result<u64> {
		self.pending_count_under(extensions, "").await
	}

	/// How many of those files sit at or under `under`, a path relative to
	/// the source root. Empty means the whole source.
	pub async fn pending_count_under(&self, extensions: &[&str], under: &str) -> Result<u64> {
		if extensions.is_empty() {
			return Ok(0);
		}
		let mut sql = format!("SELECT COUNT(*) {}", pending_clause(extensions.len()));
		let under = under.trim_matches('/');
		if !under.is_empty() {
			sql.push_str(&format!(
				" AND ({RECORD_PATH} = ? OR substr({RECORD_PATH}, 1, ?) = ?)"
			));
		}
		let mut query = sqlx::query_scalar::<_, i64>(&sql);
		for extension in extensions {
			query = query.bind(extension.to_string());
		}
		if !under.is_empty() {
			let prefix = format!("{under}/");
			query = query
				.bind(under.to_string())
				.bind(prefix.chars().count() as i64)
				.bind(prefix);
		}
		Ok(query.fetch_one(&self.pool).await?.max(0) as u64)
	}

	/// Write what a pass read, replacing any older text for the same records.
	pub async fn write(&self, docs: &[TextDoc]) -> Result<()> {
		if docs.is_empty() {
			return Ok(());
		}
		let _writer = self.writer.lock().await;
		let mut tx = self.pool.begin().await?;
		for doc in docs {
			let old: Option<i64> =
				sqlx::query_scalar("SELECT id FROM text_doc WHERE record_uuid = ?")
					.bind(doc.record_uuid)
					.fetch_optional(&mut *tx)
					.await?;
			if let Some(id) = old {
				sqlx::query("DELETE FROM text_fts WHERE rowid = ?")
					.bind(id)
					.execute(&mut *tx)
					.await?;
			}
			let id: i64 = sqlx::query_scalar(
				"INSERT INTO text_doc (record_uuid, size, mtime, extension, kind, text_bytes, truncated, skipped) \
				 VALUES (?, ?, ?, ?, ?, ?, ?, ?) \
				 ON CONFLICT (record_uuid) DO UPDATE SET size = excluded.size, \
				 mtime = excluded.mtime, extension = excluded.extension, kind = excluded.kind, text_bytes = excluded.text_bytes, \
				 truncated = excluded.truncated, skipped = excluded.skipped \
				 RETURNING id",
			)
			.bind(doc.record_uuid)
			.bind(doc.size)
			.bind(doc.mtime)
			.bind(&doc.extension)
			.bind(&doc.kind)
			.bind(doc.text.len() as i64)
			.bind(doc.truncated)
			.bind(&doc.skipped)
			.fetch_one(&mut *tx)
			.await?;
			if !doc.text.is_empty() {
				sqlx::query("INSERT INTO text_fts (rowid, body) VALUES (?, ?)")
					.bind(id)
					.bind(&doc.text)
					.execute(&mut *tx)
					.await?;
			}
		}
		tx.commit().await?;
		Ok(())
	}

	/// Remove rows whose record is gone or no longer has one of
	/// `extensions`, which a rename to another extension does.
	pub async fn prune(&self, extensions: &[&str]) -> Result<u64> {
		let placeholders = vec!["?"; extensions.len().max(1)].join(", ");
		let orphans = format!(
			"SELECT d.id FROM text_doc d \
			 LEFT JOIN src.record r ON r.uuid = d.record_uuid AND r.type = 'file' \
			 LEFT JOIN src.facet_file f ON f.record_uuid = r.uuid \
			 WHERE r.uuid IS NULL OR f.record_uuid IS NULL \
			 OR lower(f.extension) IS NULL OR lower(f.extension) NOT IN ({placeholders})"
		);
		let _writer = self.writer.lock().await;
		let mut tx = self.pool.begin().await?;
		let mut ids = sqlx::query_scalar::<_, i64>(&orphans);
		for extension in extensions {
			ids = ids.bind(extension.to_string());
		}
		if extensions.is_empty() {
			ids = ids.bind("");
		}
		let ids = ids.fetch_all(&mut *tx).await?;
		for id in &ids {
			sqlx::query("DELETE FROM text_fts WHERE rowid = ?")
				.bind(id)
				.execute(&mut *tx)
				.await?;
			sqlx::query("DELETE FROM text_doc WHERE id = ?")
				.bind(id)
				.execute(&mut *tx)
				.await?;
		}
		tx.commit().await?;
		Ok(ids.len() as u64)
	}

	/// Drop every row and give the space back. The record store is not
	/// touched; the next pass reads every file again.
	pub async fn clear(&self) -> Result<()> {
		let _writer = self.writer.lock().await;
		let mut conn = self.pool.acquire().await?;
		let mut tx = conn.begin().await?;
		for statement in [
			"DROP TABLE IF EXISTS text_fts",
			"DROP TABLE IF EXISTS text_doc",
		]
		.into_iter()
		.chain(TEXT_SCHEMA)
		{
			sqlx::query(statement).execute(&mut *tx).await?;
		}
		sqlx::query(
			"INSERT INTO text_meta (key, value) VALUES ('epoch', 1) \
			 ON CONFLICT (key) DO UPDATE SET value = value + 1",
		)
		.execute(&mut *tx)
		.await?;
		tx.commit().await?;
		sqlx::query("VACUUM").execute(&mut *conn).await?;
		Ok(())
	}

	/// A number [`Self::clear`] raises. A pass that sees it change knows
	/// the rows it wrote before are gone and starts over.
	pub async fn epoch(&self) -> Result<i64> {
		Ok(
			sqlx::query_scalar("SELECT value FROM text_meta WHERE key = 'epoch'")
				.fetch_optional(&self.pool)
				.await?
				.unwrap_or(0),
		)
	}

	pub async fn counts(&self) -> Result<TextIndexCounts> {
		let (files, truncated, skipped): (i64, Option<i64>, Option<i64>) = sqlx::query_as(
			"SELECT COUNT(*), SUM(truncated), SUM(skipped IS NOT NULL) FROM text_doc",
		)
		.fetch_one(&self.pool)
		.await?;
		Ok(TextIndexCounts {
			files: files.max(0) as u64,
			truncated: truncated.unwrap_or(0).max(0) as u64,
			skipped: skipped.unwrap_or(0).max(0) as u64,
		})
	}

	/// Ranked hits for `search`, best first.
	///
	/// Only rows whose stamp still matches the record's facet are returned,
	/// so a file changed since it was read never answers with its old text.
	/// Document hits and transcript segment hits are ranked together by
	/// BM25. A segment is short, so it tends to outrank a long document
	/// that holds the same words; order across the two is approximate.
	pub async fn search(&self, search: &TextSearch) -> Result<Vec<TextHit>> {
		if search.expression.is_empty() {
			return Ok(Vec::new());
		}
		let under = search.under.trim_matches('/');
		let want_transcripts =
			search.kinds.is_empty() || search.kinds.iter().any(|kind| kind == TRANSCRIPT_KIND);
		let text_kinds: Vec<&String> = search
			.kinds
			.iter()
			.filter(|kind| *kind != TRANSCRIPT_KIND)
			.collect();
		let want_text = search.kinds.is_empty() || !text_kinds.is_empty();

		let mut binds: Vec<Bind> = Vec::new();
		let mut branches = Vec::new();
		let snippet_tokens = i64::from(search.snippet_tokens.clamp(1, 64));
		let path = RECORD_PATH;
		let under_clause = |binds: &mut Vec<Bind>| {
			if under.is_empty() {
				return String::new();
			}
			let prefix = format!("{under}/");
			binds.push(Bind::Text(under.to_string()));
			binds.push(Bind::Int(prefix.chars().count() as i64));
			binds.push(Bind::Text(prefix));
			format!(" AND ({path} = ? OR substr({path}, 1, ?) = ?)")
		};

		if want_text {
			binds.extend([
				Bind::Text(MATCH_START.to_string()),
				Bind::Text(MATCH_END.to_string()),
				Bind::Int(snippet_tokens),
				Bind::Text(search.expression.clone()),
			]);
			let mut sql = String::from(
				"SELECT d.record_uuid AS hit_uuid, r.parent_uuid AS hit_parent, p.path AS hit_dir, \
				 r.title AS hit_title, d.kind AS hit_kind, bm25(text_fts) AS hit_rank, \
				 snippet(text_fts, 0, ?, ?, '…', ?) AS hit_snippet, d.truncated AS hit_truncated, \
				 NULL AS hit_start, NULL AS hit_end \
				 FROM text_fts \
				 JOIN text_doc d ON d.id = text_fts.rowid \
				 JOIN src.record r ON r.uuid = d.record_uuid \
				 JOIN src.facet_file f ON f.record_uuid = r.uuid \
				 LEFT JOIN src.directory_path p ON p.record_uuid = r.parent_uuid \
				 WHERE text_fts MATCH ? AND f.size IS d.size AND f.mtime IS d.mtime \
				 AND lower(f.extension) = d.extension",
			);
			sql.push_str(&under_clause(&mut binds));
			if !text_kinds.is_empty() {
				sql.push_str(&format!(
					" AND d.kind IN ({})",
					vec!["?"; text_kinds.len()].join(", ")
				));
				binds.extend(text_kinds.iter().map(|kind| Bind::Text((*kind).clone())));
			}
			branches.push(sql);
		}
		if want_transcripts {
			binds.extend([
				Bind::Text(TRANSCRIPT_KIND.to_string()),
				Bind::Text(MATCH_START.to_string()),
				Bind::Text(MATCH_END.to_string()),
				Bind::Int(snippet_tokens),
				Bind::Text(search.expression.clone()),
			]);
			let mut sql = String::from(
				"SELECT d.record_uuid AS hit_uuid, r.parent_uuid AS hit_parent, p.path AS hit_dir, \
				 r.title AS hit_title, ? AS hit_kind, bm25(transcript_fts) AS hit_rank, \
				 snippet(transcript_fts, 0, ?, ?, '…', ?) AS hit_snippet, 0 AS hit_truncated, \
				 s.start_ms AS hit_start, s.end_ms AS hit_end \
				 FROM transcript_fts \
				 JOIN transcript_span s ON s.id = transcript_fts.rowid \
				 JOIN transcript_doc d ON d.id = s.doc_id \
				 JOIN src.record r ON r.uuid = d.record_uuid \
				 JOIN src.facet_file f ON f.record_uuid = r.uuid \
				 LEFT JOIN src.directory_path p ON p.record_uuid = r.parent_uuid \
				 WHERE transcript_fts MATCH ? AND f.size IS d.size AND f.mtime IS d.mtime \
				 AND lower(f.extension) = d.extension",
			);
			sql.push_str(&under_clause(&mut binds));
			branches.push(sql);
		}
		let sql = format!(
			"SELECT * FROM ({}) ORDER BY hit_rank, hit_start LIMIT ? OFFSET ?",
			branches.join(" UNION ALL ")
		);
		binds.push(Bind::Int(search.limit as i64));
		binds.push(Bind::Int(search.offset as i64));

		let mut query = sqlx::query(&sql);
		for bind in binds {
			query = match bind {
				Bind::Text(text) => query.bind(text),
				Bind::Int(value) => query.bind(value),
			};
		}
		let rows = query
			.fetch_all(&self.pool)
			.await
			.map_err(|error| match &error {
				sqlx::Error::Database(db) if db.message().contains("fts5") => {
					Error::Other(format!("invalid search: {}", db.message()))
				}
				_ => error.into(),
			})?;

		Ok(rows
			.into_iter()
			.filter_map(|row| {
				let snippet: String = row.get(6);
				Some(TextHit {
					record_uuid: row.get(0),
					external_id: address(row.get(1), row.get(2), row.get(3))?,
					kind: row.get(4),
					score: -row.get::<f64, _>(5),
					snippet: split_snippet(&snippet),
					truncated: row.get(7),
					start_ms: row.get(8),
					end_ms: row.get(9),
				})
			})
			.collect())
	}

	/// Audio and video files past `after` with one of `extensions` whose
	/// transcript row is missing or stamped with other bytes, in row order,
	/// at or under `under` (relative to the source root; empty is the whole
	/// source).
	pub async fn pending_transcripts(
		&self,
		extensions: &[&str],
		under: &str,
		after: i64,
		limit: usize,
	) -> Result<Vec<PendingTranscript>> {
		if extensions.is_empty() {
			return Ok(Vec::new());
		}
		let mut sql = format!(
			"SELECT r.rowid, r.uuid, r.parent_uuid, p.path, r.title, f.size, f.mtime, \
			 lower(f.extension), COALESCE(c.candidate_uuid, c.uuid) \
			 FROM src.record r \
			 JOIN src.facet_file f ON f.record_uuid = r.uuid \
			 LEFT JOIN src.directory_path p ON p.record_uuid = r.parent_uuid \
			 LEFT JOIN src.content c ON c.id = r.content_id \
			 LEFT JOIN transcript_doc d ON d.record_uuid = r.uuid \
			 WHERE r.type = 'file' AND f.link_target IS NULL AND lower(f.extension) IN ({}) \
			 AND (d.id IS NULL OR d.size IS NOT f.size OR d.mtime IS NOT f.mtime \
			 OR d.extension IS NOT lower(f.extension)) AND r.rowid > ?",
			vec!["?"; extensions.len()].join(", ")
		);
		let under = under.trim_matches('/');
		if !under.is_empty() {
			sql.push_str(&format!(
				" AND ({RECORD_PATH} = ? OR substr({RECORD_PATH}, 1, ?) = ?)"
			));
		}
		sql.push_str(" ORDER BY r.rowid LIMIT ?");
		let mut query = sqlx::query(&sql);
		for extension in extensions {
			query = query.bind(extension.to_string());
		}
		query = query.bind(after);
		if !under.is_empty() {
			let prefix = format!("{under}/");
			query = query
				.bind(under.to_string())
				.bind(prefix.chars().count() as i64)
				.bind(prefix);
		}
		let rows = query.bind(limit as i64).fetch_all(&self.pool).await?;
		Ok(rows
			.into_iter()
			.filter_map(|row| {
				Some(PendingTranscript {
					rowid: row.get(0),
					record_uuid: row.get(1),
					external_id: address(row.get(2), row.get(3), row.get(4))?,
					size: row.get(5),
					mtime: row.get(6),
					extension: row.get(7),
					content_uuid: row.get(8),
				})
			})
			.collect())
	}

	/// Write a file's transcript, replacing any older one for the record.
	pub async fn write_transcript(&self, doc: &TranscriptDoc) -> Result<()> {
		let _writer = self.writer.lock().await;
		let mut tx = self.pool.begin().await?;
		let old: Option<i64> =
			sqlx::query_scalar("SELECT id FROM transcript_doc WHERE record_uuid = ?")
				.bind(doc.record_uuid)
				.fetch_optional(&mut *tx)
				.await?;
		if let Some(id) = old {
			delete_spans(&mut tx, id).await?;
		}
		let id: i64 = sqlx::query_scalar(
			"INSERT INTO transcript_doc (record_uuid, size, mtime, extension, language, model, duration_ms, skipped) \
			 VALUES (?, ?, ?, ?, ?, ?, ?, ?) \
			 ON CONFLICT (record_uuid) DO UPDATE SET size = excluded.size, mtime = excluded.mtime, \
			 extension = excluded.extension, language = excluded.language, model = excluded.model, \
			 duration_ms = excluded.duration_ms, skipped = excluded.skipped \
			 RETURNING id",
		)
		.bind(doc.record_uuid)
		.bind(doc.size)
		.bind(doc.mtime)
		.bind(&doc.extension)
		.bind(&doc.language)
		.bind(&doc.model)
		.bind(doc.duration_ms)
		.bind(&doc.skipped)
		.fetch_one(&mut *tx)
		.await?;
		for span in doc.spans.iter().filter(|span| !span.text.trim().is_empty()) {
			let span_id: i64 = sqlx::query_scalar(
				"INSERT INTO transcript_span (doc_id, start_ms, end_ms) VALUES (?, ?, ?) RETURNING id",
			)
			.bind(id)
			.bind(span.start_ms)
			.bind(span.end_ms)
			.fetch_one(&mut *tx)
			.await?;
			sqlx::query("INSERT INTO transcript_fts (rowid, body) VALUES (?, ?)")
				.bind(span_id)
				.bind(&span.text)
				.execute(&mut *tx)
				.await?;
		}
		tx.commit().await?;
		Ok(())
	}

	/// Drop the transcripts of files at or under `under`, so the next pass
	/// makes them again. Empty means the whole source. Returns how many.
	pub async fn forget_transcripts(&self, under: &str) -> Result<u64> {
		let under = under.trim_matches('/');
		let mut sql = String::from(
			"SELECT d.id FROM transcript_doc d \
			 LEFT JOIN src.record r ON r.uuid = d.record_uuid \
			 LEFT JOIN src.directory_path p ON p.record_uuid = r.parent_uuid",
		);
		if !under.is_empty() {
			sql.push_str(&format!(
				" WHERE ({RECORD_PATH} = ? OR substr({RECORD_PATH}, 1, ?) = ?)"
			));
		}
		self.delete_transcripts(&sql, under).await
	}

	/// Remove transcripts whose record is gone.
	pub async fn prune_transcripts(&self) -> Result<u64> {
		self.delete_transcripts(
			"SELECT d.id FROM transcript_doc d \
			 LEFT JOIN src.record r ON r.uuid = d.record_uuid AND r.type = 'file' \
			 WHERE r.uuid IS NULL",
			"",
		)
		.await
	}

	/// How many files have a transcript row, spoken words or not.
	pub async fn transcript_count(&self) -> Result<u64> {
		let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM transcript_doc")
			.fetch_one(&self.pool)
			.await?;
		Ok(count.max(0) as u64)
	}

	async fn delete_transcripts(&self, select_ids: &str, under: &str) -> Result<u64> {
		let _writer = self.writer.lock().await;
		let mut tx = self.pool.begin().await?;
		let mut ids = sqlx::query_scalar::<_, i64>(select_ids);
		if !under.is_empty() {
			let prefix = format!("{under}/");
			ids = ids
				.bind(under.to_string())
				.bind(prefix.chars().count() as i64)
				.bind(prefix);
		}
		let ids = ids.fetch_all(&mut *tx).await?;
		for id in &ids {
			delete_spans(&mut tx, *id).await?;
			sqlx::query("DELETE FROM transcript_doc WHERE id = ?")
				.bind(id)
				.execute(&mut *tx)
				.await?;
		}
		tx.commit().await?;
		Ok(ids.len() as u64)
	}
}

/// A value bound into a query assembled from optional parts.
enum Bind {
	Text(String),
	Int(i64),
}

async fn delete_spans(tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>, doc_id: i64) -> Result<()> {
	sqlx::query(
		"DELETE FROM transcript_fts WHERE rowid IN (SELECT id FROM transcript_span WHERE doc_id = ?)",
	)
	.bind(doc_id)
	.execute(&mut **tx)
	.await?;
	sqlx::query("DELETE FROM transcript_span WHERE doc_id = ?")
		.bind(doc_id)
		.execute(&mut **tx)
		.await?;
	Ok(())
}

/// A record's path relative to the source root, over `r` and its parent's
/// `p`.
const RECORD_PATH: &str =
	"(CASE WHEN r.parent_uuid IS NULL THEN r.title ELSE p.path || '/' || r.title END)";

/// The rows a pass has to read, after `FROM`, with one placeholder per
/// extension.
fn pending_clause(extensions: usize) -> String {
	format!(
		"FROM src.record r \
		 JOIN src.facet_file f ON f.record_uuid = r.uuid \
		 LEFT JOIN src.directory_path p ON p.record_uuid = r.parent_uuid \
		 LEFT JOIN text_doc d ON d.record_uuid = r.uuid \
		 WHERE r.type = 'file' AND f.link_target IS NULL AND lower(f.extension) IN ({}) \
		 AND (d.id IS NULL OR d.size IS NOT f.size OR d.mtime IS NOT f.mtime \
		 OR d.extension IS NOT lower(f.extension))",
		vec!["?"; extensions].join(", ")
	)
}

/// An FTS5 expression for what a person typed.
///
/// Words are matched as words and every word must appear. Text in double
/// quotes is a phrase. A word ending in `*` matches as a prefix. Every
/// other character is literal, so nothing typed can reach FTS5's own query
/// syntax.
pub fn match_expression(query: &str) -> String {
	let mut terms = Vec::new();
	for (i, part) in query.split('"').enumerate() {
		if i % 2 == 1 {
			let words: Vec<&str> = part.split_whitespace().collect();
			if !words.is_empty() {
				terms.push(quote(&words.join(" ")));
			}
			continue;
		}
		for word in part.split_whitespace() {
			match word.strip_suffix('*') {
				Some(stem) if !stem.trim_end_matches('*').is_empty() => {
					terms.push(format!("{}*", quote(stem.trim_end_matches('*'))))
				}
				Some(_) => {}
				None => terms.push(quote(word)),
			}
		}
	}
	terms.join(" AND ")
}

fn quote(text: &str) -> String {
	format!("\"{}\"", text.replace('"', "\"\""))
}

fn split_snippet(snippet: &str) -> Vec<SnippetPart> {
	let mut parts = Vec::new();
	let mut current = String::new();
	let mut matched = false;
	let mut push = |text: &mut String, matched: bool| {
		if !text.is_empty() {
			parts.push(SnippetPart {
				text: std::mem::take(text),
				matched,
			});
		}
	};
	for c in snippet.chars() {
		match c {
			MATCH_START => {
				push(&mut current, matched);
				matched = true;
			}
			MATCH_END => {
				push(&mut current, matched);
				matched = false;
			}
			c => current.push(c),
		}
	}
	push(&mut current, matched);
	parts
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn typed_text_never_reaches_fts_syntax() {
		assert_eq!(match_expression("needle"), r#""needle""#);
		assert_eq!(match_expression("a b"), r#""a" AND "b""#);
		assert_eq!(
			match_expression(r#""exact  phrase" other"#),
			r#""exact phrase" AND "other""#
		);
		assert_eq!(match_expression("pre*"), r#""pre"*"#);
		assert_eq!(match_expression("*"), "");
		assert_eq!(
			match_expression("NOT OR AND"),
			r#""NOT" AND "OR" AND "AND""#
		);
		assert_eq!(match_expression("a(b) c:d"), r#""a(b)" AND "c:d""#);
		assert_eq!(match_expression(r#"say "hi"#), r#""say" AND "hi""#);
		assert_eq!(match_expression("   "), "");
	}

	#[test]
	fn a_snippet_splits_at_its_markers() {
		let snippet = format!("one {MATCH_START}two{MATCH_END} three");
		assert_eq!(
			split_snippet(&snippet),
			vec![
				SnippetPart {
					text: "one ".into(),
					matched: false
				},
				SnippetPart {
					text: "two".into(),
					matched: true
				},
				SnippetPart {
					text: " three".into(),
					matched: false
				},
			]
		);
	}
}
