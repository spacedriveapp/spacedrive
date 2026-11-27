//! # Space versions
//!
//! The history of a space with versioning on: the bytes of every version of
//! every file, and which version each path held at each point. One database
//! per space at `<store dir>/spaces/<space id>/versions.db`, beside the
//! space's event log, with the bytes under `blobs/` in the same directory.
//! Nothing here is written for a space with versioning off, and nothing is
//! written outside the space's directory.
//!
//! A file's bytes are cut into chunks with FastCDC (about 64 KiB on average)
//! and each chunk is stored once, named by its BLAKE3 hash, so a version that
//! changes part of a large file stores only the chunks that changed. A blob
//! is a file's content: its BLAKE3 hash, the same hash agent write
//! preconditions name, and the list of chunks that rebuild it.
//!
//! The `version` table is the space's manifest kept as a delta log. Each row
//! says what one path holds from that row on: a blob, or nothing for a
//! delete. A row's id is a manifest id, and the manifest at an id is the
//! newest row per path at or below it. A snapshot is a name for one manifest
//! id, so it copies nothing. Per-file history is the rows for one path.
//!
//! Retention keeps every row newer than the cutoff, the newest row of every
//! path, and the rows each snapshot's manifest needs. Blobs and chunks no
//! kept row reaches are then removed. A capture and a prune never overlap,
//! so a chunk a capture is about to reuse is never collected under it.

use std::{
	collections::HashSet,
	io::{Read, Write},
	path::{Path, PathBuf},
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
};

use fastcdc::v2020::StreamCDC;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::{Connection, SqlitePool};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::error::{Error, Result};

/// The file name of a space's version database inside its directory.
pub const SPACE_VERSIONS_FILE: &str = "versions.db";

/// The directory beside it that holds the chunks.
pub const BLOBS_DIR: &str = "blobs";

/// FastCDC bounds. The average sets the dedupe grain: an edit costs about
/// one average chunk plus the bytes it changed.
pub const CHUNK_MIN: u32 = 16 * 1024;
pub const CHUNK_AVG: u32 = 64 * 1024;
pub const CHUNK_MAX: u32 = 256 * 1024;

const HASH_LEN: usize = blake3::OUT_LEN;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS blob (
    hash TEXT PRIMARY KEY,
    size INTEGER NOT NULL,
    chunks BLOB NOT NULL
);
CREATE TABLE IF NOT EXISTS version (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT NOT NULL,
    blob_hash TEXT,
    size INTEGER,
    mode INTEGER,
    mtime TEXT,
    at TEXT NOT NULL,
    actor_id BLOB NOT NULL,
    origin TEXT NOT NULL,
    event_seq INTEGER
);
CREATE INDEX IF NOT EXISTS idx_version_path ON version(path, id);
CREATE INDEX IF NOT EXISTS idx_version_blob ON version(blob_hash);
CREATE TABLE IF NOT EXISTS snapshot (
    id BLOB PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    manifest_id INTEGER NOT NULL,
    created_by BLOB NOT NULL,
    at TEXT NOT NULL
);
"#;

const VERSION_COLUMNS: &str =
	"id, path, blob_hash, size, mode, mtime, at, actor_id, origin, event_seq";

/// What one path holds from this row on.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct VersionRow {
	/// The manifest id. Assigned on append; ignored when appending.
	pub id: i64,
	/// Relative to the space root, `/`-separated.
	pub path: String,
	/// The content, or None when the path was deleted.
	pub blob_hash: Option<String>,
	pub size: Option<i64>,
	/// Unix permission bits, when the platform has them.
	pub mode: Option<i64>,
	/// RFC 3339 modification time of the file when it was captured.
	pub mtime: Option<String>,
	/// RFC 3339 time of the capture.
	pub at: String,
	pub actor_id: Uuid,
	/// The origin of the change as the event log names it, or `baseline` for
	/// a file that was already there when versioning was turned on.
	pub origin: String,
	/// The space event that made this version, when there was one.
	pub event_seq: Option<i64>,
}

impl VersionRow {
	/// A row with no path or content yet, carrying the attribution.
	pub fn attributed(at: &str, actor_id: Uuid, origin: &str, event_seq: Option<i64>) -> Self {
		Self {
			id: 0,
			path: String::new(),
			blob_hash: None,
			size: None,
			mode: None,
			mtime: None,
			at: at.to_string(),
			actor_id,
			origin: origin.to_string(),
			event_seq,
		}
	}
}

/// A named, pinned manifest.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SnapshotRow {
	pub id: Uuid,
	pub name: String,
	pub manifest_id: i64,
	pub created_by: Uuid,
	/// RFC 3339.
	pub at: String,
}

/// A file's bytes as the store holds them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredBlob {
	/// BLAKE3 of the whole file, hex.
	pub hash: String,
	pub size: u64,
	pub chunks: usize,
	/// Chunks this capture wrote because no earlier version had them.
	pub new_chunks: usize,
	pub new_bytes: u64,
}

/// What a prune removed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Pruned {
	pub versions: u64,
	pub blobs: u64,
	pub chunks: u64,
	pub bytes: u64,
}

/// One space's versions, open for captures, reads and pruning.
#[derive(Debug, Clone)]
pub struct SpaceVersions {
	pool: SqlitePool,
	blobs: PathBuf,
	/// Captures share it; a prune takes it alone.
	collect: Arc<RwLock<()>>,
	/// Set by [`Self::close`]; a capture after it stores nothing.
	closed: Arc<AtomicBool>,
}

impl SpaceVersions {
	/// Open the versions of the space whose directory is `dir`, creating the
	/// database and the chunk directory.
	pub async fn open(dir: &Path) -> Result<Self> {
		let blobs = dir.join(BLOBS_DIR);
		tokio::fs::create_dir_all(&blobs).await?;
		let options = SqliteConnectOptions::new()
			.filename(dir.join(SPACE_VERSIONS_FILE))
			.create_if_missing(true)
			.journal_mode(SqliteJournalMode::Wal)
			.busy_timeout(std::time::Duration::from_secs(5));
		let pool = SqlitePoolOptions::new()
			.max_connections(2)
			.connect_with(options)
			.await?;
		sqlx::raw_sql(SCHEMA).execute(&pool).await?;
		Ok(Self {
			pool,
			blobs,
			collect: Arc::new(RwLock::new(())),
			closed: Arc::new(AtomicBool::new(false)),
		})
	}

	/// Wait for captures and prunes in flight, then refuse every later one
	/// and close the database, so the files can be deleted with nothing
	/// still writing to them.
	pub async fn close(&self) {
		let _alone = self.collect.write().await;
		self.closed.store(true, Ordering::SeqCst);
		self.pool.close().await;
	}

	/// Whether [`Self::close`] ran.
	pub fn is_closed(&self) -> bool {
		self.closed.load(Ordering::SeqCst)
	}

	/// The pool, for an export that copies the tables.
	pub fn pool(&self) -> &SqlitePool {
		&self.pool
	}

	/// Store the file at `file` and record it as the new content of
	/// `row.path`, unless that path holds these bytes already. Answers the
	/// new row's id and the stored blob, or None when nothing changed.
	pub async fn capture(
		&self,
		file: &Path,
		mut row: VersionRow,
	) -> Result<Option<(i64, StoredBlob)>> {
		let _shared = self.collect.read().await;
		if self.is_closed() {
			return Err(Error::Other("the space's versions were closed".to_string()));
		}
		let meta = tokio::fs::metadata(file).await?;
		if !meta.is_file() {
			return Ok(None);
		}
		let stored = self.store_file(file).await?;
		if let Some(current) = self.current(&row.path).await? {
			if current.blob_hash.as_deref() == Some(stored.hash.as_str()) {
				return Ok(None);
			}
		}
		row.blob_hash = Some(stored.hash.clone());
		row.size = Some(stored.size as i64);
		row.mode = mode_of(&meta);
		row.mtime = meta
			.modified()
			.ok()
			.map(|at| chrono::DateTime::<chrono::Utc>::from(at).to_rfc3339());
		let id = self.append(&row).await?;
		Ok(Some((id, stored)))
	}

	/// Chunk and store a file's bytes, writing only the chunks not stored
	/// yet. Only [`Self::capture`] calls it, under the shared lock, so the
	/// blob is referenced before a prune can look for orphans.
	async fn store_file(&self, file: &Path) -> Result<StoredBlob> {
		let (blobs, file) = (self.blobs.clone(), file.to_path_buf());
		let (stored, list) = tokio::task::spawn_blocking(move || chunk_into(&blobs, &file))
			.await
			.map_err(|e| Error::Other(format!("chunk a file: {e}")))??;
		sqlx::query("INSERT OR IGNORE INTO blob (hash, size, chunks) VALUES (?, ?, ?)")
			.bind(&stored.hash)
			.bind(stored.size as i64)
			.bind(list)
			.execute(&self.pool)
			.await?;
		Ok(stored)
	}

	/// Append one row as given, and answer its id.
	pub async fn append(&self, row: &VersionRow) -> Result<i64> {
		let mut conn = self.pool.acquire().await?;
		insert(&mut conn, row).await
	}

	/// Record that `from` and everything under it now lives at `to`: a
	/// delete at each old path and the same content at each new one.
	/// Answers how many files moved.
	pub async fn moved(&self, from: &str, to: &str, template: &VersionRow) -> Result<usize> {
		let entries = self.manifest(None, Some(from)).await?;
		let mut conn = self.pool.acquire().await?;
		let mut tx = conn.begin().await?;
		for entry in &entries {
			let rest = &entry.path[from.len()..];
			insert(
				&mut tx,
				&VersionRow {
					path: entry.path.clone(),
					..template.clone()
				},
			)
			.await?;
			insert(
				&mut tx,
				&VersionRow {
					path: format!("{to}{rest}"),
					blob_hash: entry.blob_hash.clone(),
					size: entry.size,
					mode: entry.mode,
					mtime: entry.mtime.clone(),
					..template.clone()
				},
			)
			.await?;
		}
		tx.commit().await?;
		Ok(entries.len())
	}

	/// Record that `path` and everything under it is gone. Answers how many
	/// files were removed.
	pub async fn removed(&self, path: &str, template: &VersionRow) -> Result<usize> {
		let entries = self.manifest(None, Some(path)).await?;
		let mut conn = self.pool.acquire().await?;
		let mut tx = conn.begin().await?;
		for entry in &entries {
			insert(
				&mut tx,
				&VersionRow {
					path: entry.path.clone(),
					..template.clone()
				},
			)
			.await?;
		}
		tx.commit().await?;
		Ok(entries.len())
	}

	/// The newest row for one path, a delete included.
	pub async fn current(&self, path: &str) -> Result<Option<VersionRow>> {
		Ok(sqlx::query_as(&format!(
			"SELECT {VERSION_COLUMNS} FROM version WHERE path = ? ORDER BY id DESC LIMIT 1"
		))
		.bind(path)
		.fetch_optional(&self.pool)
		.await?)
	}

	/// One row by id.
	pub async fn version(&self, id: i64) -> Result<Option<VersionRow>> {
		Ok(sqlx::query_as(&format!(
			"SELECT {VERSION_COLUMNS} FROM version WHERE id = ?"
		))
		.bind(id)
		.fetch_optional(&self.pool)
		.await?)
	}

	/// The newest manifest id, or 0 before the first capture.
	pub async fn head(&self) -> Result<i64> {
		Ok(
			sqlx::query_scalar("SELECT COALESCE(MAX(id), 0) FROM version")
				.fetch_one(&self.pool)
				.await?,
		)
	}

	/// Rows newest first, below the `before` id when given, for one path and
	/// everything under it, or for the whole space.
	pub async fn history(
		&self,
		path: Option<&str>,
		before: Option<i64>,
		limit: u32,
	) -> Result<Vec<VersionRow>> {
		let (filter, prefix) = under(path, 3);
		let sql = format!(
			"SELECT {VERSION_COLUMNS} FROM version WHERE id < ?1 {filter} ORDER BY id DESC LIMIT ?2"
		);
		let mut query = sqlx::query_as(&sql)
			.bind(before.unwrap_or(i64::MAX))
			.bind(limit);
		if let Some(prefix) = prefix {
			query = query
				.bind(prefix.clone())
				.bind(format!("{prefix}/"))
				.bind(format!("{prefix}0"));
		}
		Ok(query.fetch_all(&self.pool).await?)
	}

	/// The files in the manifest at `at` (the newest when None), under
	/// `path` when given, by path.
	pub async fn manifest(&self, at: Option<i64>, path: Option<&str>) -> Result<Vec<VersionRow>> {
		let (filter, prefix) = under(path, 2);
		let sql = format!(
			"SELECT {VERSION_COLUMNS} FROM version WHERE id IN (
				SELECT MAX(id) FROM version WHERE id <= ?1 {filter} GROUP BY path
			) AND blob_hash IS NOT NULL ORDER BY path"
		);
		let mut query = sqlx::query_as(&sql).bind(at.unwrap_or(i64::MAX));
		if let Some(prefix) = prefix {
			query = query
				.bind(prefix.clone())
				.bind(format!("{prefix}/"))
				.bind(format!("{prefix}0"));
		}
		Ok(query.fetch_all(&self.pool).await?)
	}

	/// Write the blob `hash` to `dest`, creating or truncating it, and check
	/// every chunk and the whole file against their hashes. Answers the size.
	pub async fn read_blob_to(&self, hash: &str, dest: &Path) -> Result<u64> {
		let chunks = self.chunks_of(hash).await?;
		let (blobs, dest, hash) = (self.blobs.clone(), dest.to_path_buf(), hash.to_string());
		tokio::task::spawn_blocking(move || {
			let mut file = std::fs::File::create(&dest)?;
			let size = copy_chunks(&blobs, &chunks, &hash, &mut file)?;
			file.flush()?;
			Ok(size)
		})
		.await
		.map_err(|e| Error::Other(format!("read a blob: {e}")))?
	}

	/// The chunk hashes that rebuild a blob, in order.
	pub async fn chunks_of(&self, hash: &str) -> Result<Vec<[u8; HASH_LEN]>> {
		let list: Vec<u8> = sqlx::query_scalar("SELECT chunks FROM blob WHERE hash = ?")
			.bind(hash)
			.fetch_optional(&self.pool)
			.await?
			.ok_or_else(|| Error::Other(format!("no stored content {hash}")))?;
		Ok(split_chunks(&list))
	}

	/// The path of one chunk on disk.
	pub fn chunk_path(&self, chunk: &[u8; HASH_LEN]) -> PathBuf {
		chunk_path(&self.blobs, chunk)
	}

	/// Pin the newest manifest under `name`.
	pub async fn create_snapshot(
		&self,
		name: &str,
		created_by: Uuid,
		at: &str,
	) -> Result<SnapshotRow> {
		let row = SnapshotRow {
			id: Uuid::now_v7(),
			name: name.to_string(),
			manifest_id: self.head().await?,
			created_by,
			at: at.to_string(),
		};
		self.put_snapshot(&row).await?;
		Ok(row)
	}

	/// Write a snapshot row as given; a taken name is refused.
	pub async fn put_snapshot(&self, row: &SnapshotRow) -> Result<()> {
		let inserted = sqlx::query(
			"INSERT INTO snapshot (id, name, manifest_id, created_by, at) VALUES (?, ?, ?, ?, ?)
			 ON CONFLICT DO NOTHING",
		)
		.bind(row.id)
		.bind(&row.name)
		.bind(row.manifest_id)
		.bind(row.created_by)
		.bind(&row.at)
		.execute(&self.pool)
		.await?;
		if inserted.rows_affected() == 0 {
			return Err(Error::Other(format!(
				"a snapshot named \"{}\" already exists",
				row.name
			)));
		}
		Ok(())
	}

	/// Every snapshot, oldest first.
	pub async fn snapshots(&self) -> Result<Vec<SnapshotRow>> {
		Ok(sqlx::query_as(
			"SELECT id, name, manifest_id, created_by, at FROM snapshot ORDER BY manifest_id, at",
		)
		.fetch_all(&self.pool)
		.await?)
	}

	/// One snapshot by id or by name.
	pub async fn snapshot(&self, id_or_name: &str) -> Result<Option<SnapshotRow>> {
		let id = Uuid::parse_str(id_or_name).ok();
		Ok(sqlx::query_as(
			"SELECT id, name, manifest_id, created_by, at FROM snapshot WHERE id = ? OR name = ?",
		)
		.bind(id)
		.bind(id_or_name)
		.fetch_optional(&self.pool)
		.await?)
	}

	/// Unpin a snapshot. Its versions go at a later prune once retention no
	/// longer keeps them. Answers whether it existed.
	pub async fn delete_snapshot(&self, id: Uuid) -> Result<bool> {
		Ok(sqlx::query("DELETE FROM snapshot WHERE id = ?")
			.bind(id)
			.execute(&self.pool)
			.await?
			.rows_affected()
			> 0)
	}

	/// Drop the rows retention no longer keeps, then the blobs and chunks no
	/// kept row reaches. `cutoff` is RFC 3339.
	pub async fn prune(&self, cutoff: &str) -> Result<Pruned> {
		let _alone = self.collect.write().await;
		let mut pruned = Pruned::default();
		let pinned = "SELECT MAX(v.id) FROM version v JOIN snapshot s ON v.id <= s.manifest_id
			GROUP BY s.id, v.path";
		pruned.versions += sqlx::query(&format!(
			"DELETE FROM version WHERE at < ?1
				AND id NOT IN (SELECT MAX(id) FROM version GROUP BY path)
				AND id NOT IN ({pinned})"
		))
		.bind(cutoff)
		.execute(&self.pool)
		.await?
		.rows_affected();
		// A delete that is all that is left of a path says nothing anymore,
		// unless a snapshot's manifest needs it to hide an older row.
		pruned.versions += sqlx::query(&format!(
			"DELETE FROM version WHERE at < ?1 AND blob_hash IS NULL
				AND (SELECT COUNT(*) FROM version other WHERE other.path = version.path) = 1
				AND id NOT IN ({pinned})"
		))
		.bind(cutoff)
		.execute(&self.pool)
		.await?
		.rows_affected();

		let orphans: Vec<(String, Vec<u8>)> = sqlx::query_as(
			"SELECT hash, chunks FROM blob WHERE hash NOT IN
				(SELECT blob_hash FROM version WHERE blob_hash IS NOT NULL)",
		)
		.fetch_all(&self.pool)
		.await?;
		if orphans.is_empty() {
			return Ok(pruned);
		}
		for (hash, _) in &orphans {
			sqlx::query("DELETE FROM blob WHERE hash = ?")
				.bind(hash)
				.execute(&self.pool)
				.await?;
		}
		pruned.blobs = orphans.len() as u64;
		let live: Vec<Vec<u8>> = sqlx::query_scalar("SELECT chunks FROM blob")
			.fetch_all(&self.pool)
			.await?;
		let live: HashSet<[u8; HASH_LEN]> =
			live.iter().flat_map(|list| split_chunks(list)).collect();
		let dead: HashSet<[u8; HASH_LEN]> = orphans
			.iter()
			.flat_map(|(_, list)| split_chunks(list))
			.filter(|chunk| !live.contains(chunk))
			.collect();
		let blobs = self.blobs.clone();
		let (chunks, bytes) = tokio::task::spawn_blocking(move || {
			let (mut chunks, mut bytes) = (0, 0);
			for chunk in dead {
				let path = chunk_path(&blobs, &chunk);
				let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
				if std::fs::remove_file(&path).is_ok() {
					chunks += 1;
					bytes += size;
				}
			}
			(chunks, bytes)
		})
		.await
		.map_err(|e| Error::Other(format!("remove chunks: {e}")))?;
		pruned.chunks = chunks;
		pruned.bytes = bytes;
		Ok(pruned)
	}
}

/// A filter for `path` and everything under it, with its binds numbered
/// from `first`. Answers the SQL and the prefix to bind as three values.
fn under(path: Option<&str>, first: usize) -> (String, Option<String>) {
	let prefix = path
		.map(|path| path.trim_matches('/'))
		.filter(|path| !path.is_empty());
	match prefix {
		// Every path below a folder sorts between "folder/" and "folder0",
		// since '0' is the byte after '/', so the path index serves it.
		Some(prefix) => (
			format!(
				"AND (path = ?{first} OR (path > ?{} AND path < ?{}))",
				first + 1,
				first + 2
			),
			Some(prefix.to_string()),
		),
		None => (String::new(), None),
	}
}

async fn insert(conn: &mut sqlx::SqliteConnection, row: &VersionRow) -> Result<i64> {
	Ok(sqlx::query(
		"INSERT INTO version (path, blob_hash, size, mode, mtime, at, actor_id, origin, event_seq)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
	)
	.bind(&row.path)
	.bind(&row.blob_hash)
	.bind(row.size)
	.bind(row.mode)
	.bind(&row.mtime)
	.bind(&row.at)
	.bind(row.actor_id)
	.bind(&row.origin)
	.bind(row.event_seq)
	.execute(&mut *conn)
	.await?
	.last_insert_rowid())
}

#[cfg(unix)]
fn mode_of(meta: &std::fs::Metadata) -> Option<i64> {
	use std::os::unix::fs::PermissionsExt;
	Some(i64::from(meta.permissions().mode() & 0o7777))
}

#[cfg(not(unix))]
fn mode_of(_meta: &std::fs::Metadata) -> Option<i64> {
	None
}

fn split_chunks(list: &[u8]) -> Vec<[u8; HASH_LEN]> {
	list.chunks_exact(HASH_LEN)
		.map(|chunk| chunk.try_into().expect("an exact chunk"))
		.collect()
}

fn chunk_path(blobs: &Path, chunk: &[u8; HASH_LEN]) -> PathBuf {
	let hex = blake3::Hash::from_bytes(*chunk).to_hex();
	blobs.join(&hex[..2]).join(hex.as_str())
}

/// Cut a file into chunks, store the ones not stored yet, and answer the
/// blob and its chunk list.
fn chunk_into(blobs: &Path, file: &Path) -> Result<(StoredBlob, Vec<u8>)> {
	let reader = std::fs::File::open(file)?;
	let mut whole = blake3::Hasher::new();
	let mut list = Vec::new();
	let (mut size, mut chunks, mut new_chunks, mut new_bytes) = (0u64, 0, 0, 0u64);
	for chunk in StreamCDC::new(reader, CHUNK_MIN, CHUNK_AVG, CHUNK_MAX) {
		let chunk = chunk.map_err(|e| match e {
			fastcdc::v2020::Error::IoError(e) => Error::Io(e),
			other => Error::Other(format!("chunk {}: {other}", file.display())),
		})?;
		whole.update(&chunk.data);
		let hash = *blake3::hash(&chunk.data).as_bytes();
		if write_chunk(blobs, &hash, &chunk.data)? {
			new_chunks += 1;
			new_bytes += chunk.data.len() as u64;
		}
		size += chunk.data.len() as u64;
		chunks += 1;
		list.extend_from_slice(&hash);
	}
	Ok((
		StoredBlob {
			hash: whole.finalize().to_hex().to_string(),
			size,
			chunks,
			new_chunks,
			new_bytes,
		},
		list,
	))
}

/// Store one chunk unless it is stored already. It is written beside its
/// final name and renamed, so a chunk file is whole or absent. Answers
/// whether it was written.
fn write_chunk(blobs: &Path, hash: &[u8; HASH_LEN], data: &[u8]) -> Result<bool> {
	let path = chunk_path(blobs, hash);
	if path.exists() {
		return Ok(false);
	}
	let dir = path.parent().expect("a chunk path has a parent");
	std::fs::create_dir_all(dir)?;
	let partial = dir.join(format!(".{}.partial", Uuid::new_v4()));
	std::fs::write(&partial, data)?;
	if let Err(error) = std::fs::rename(&partial, &path) {
		let _ = std::fs::remove_file(&partial);
		return Err(error.into());
	}
	Ok(true)
}

/// Write the chunks in order to `out`, checking each and the whole.
fn copy_chunks(
	blobs: &Path,
	chunks: &[[u8; HASH_LEN]],
	hash: &str,
	out: &mut impl Write,
) -> Result<u64> {
	let mut whole = blake3::Hasher::new();
	let mut size = 0u64;
	let mut data = Vec::new();
	for chunk in chunks {
		data.clear();
		std::fs::File::open(chunk_path(blobs, chunk))?.read_to_end(&mut data)?;
		if blake3::hash(&data).as_bytes() != chunk {
			return Err(Error::Other(format!("a stored chunk of {hash} is damaged")));
		}
		whole.update(&data);
		out.write_all(&data)?;
		size += data.len() as u64;
	}
	if whole.finalize().to_hex().as_str() != hash {
		return Err(Error::Other(format!(
			"stored content {hash} does not match its hash"
		)));
	}
	Ok(size)
}

#[cfg(test)]
mod tests {
	use super::*;

	const ACTOR: Uuid = Uuid::from_u128(7);

	fn row(path: &str, at: &str) -> VersionRow {
		VersionRow {
			path: path.into(),
			..VersionRow::attributed(at, ACTOR, "agent_write", None)
		}
	}

	fn pseudo_random(len: usize, seed: u64) -> Vec<u8> {
		let mut state = seed | 1;
		(0..len)
			.map(|_| {
				state ^= state << 13;
				state ^= state >> 7;
				state ^= state << 17;
				state as u8
			})
			.collect()
	}

	async fn open() -> (tempfile::TempDir, SpaceVersions) {
		let dir = tempfile::tempdir().unwrap();
		let versions = SpaceVersions::open(&dir.path().join("spaces/x"))
			.await
			.unwrap();
		(dir, versions)
	}

	#[tokio::test]
	async fn an_edit_to_a_large_file_stores_only_the_chunks_it_changed() {
		let (dir, versions) = open().await;
		let file = dir.path().join("video.bin");
		let mut bytes = pseudo_random(16 * 1024 * 1024, 42);
		std::fs::write(&file, &bytes).unwrap();
		let (first, stored) = versions
			.capture(&file, row("video.bin", "2026-11-27T00:00:00Z"))
			.await
			.unwrap()
			.unwrap();
		assert_eq!(stored.size, bytes.len() as u64);
		assert_eq!(stored.hash, blake3::hash(&bytes).to_hex().as_str());
		assert!(stored.chunks > 128, "about 64 KiB per chunk: {stored:?}");

		assert!(
			versions
				.capture(&file, row("video.bin", "2026-11-27T00:00:01Z"))
				.await
				.unwrap()
				.is_none(),
			"the same bytes again are not a new version"
		);

		bytes[10 * 1024 * 1024..10 * 1024 * 1024 + 100].fill(0);
		std::fs::write(&file, &bytes).unwrap();
		let (second, edited) = versions
			.capture(&file, row("video.bin", "2026-11-27T00:00:02Z"))
			.await
			.unwrap()
			.unwrap();
		assert!(second > first);
		assert!(
			edited.new_chunks <= 2 && edited.new_bytes <= 2 * CHUNK_MAX as u64,
			"a 100 byte edit stores a chunk or two: {edited:?}"
		);

		let out = dir.path().join("restored.bin");
		let first_row = versions.version(first).await.unwrap().unwrap();
		versions
			.read_blob_to(first_row.blob_hash.as_deref().unwrap(), &out)
			.await
			.unwrap();
		assert_eq!(
			blake3::hash(&std::fs::read(&out).unwrap())
				.to_hex()
				.as_str(),
			stored.hash
		);
	}

	#[tokio::test]
	async fn the_manifest_follows_moves_and_deletes_and_snapshots_pin_it() {
		let (dir, versions) = open().await;
		let a = dir.path().join("a");
		std::fs::write(&a, "alpha").unwrap();
		versions
			.capture(&a, row("docs/a.md", "2026-11-27T00:00:00Z"))
			.await
			.unwrap();
		versions
			.capture(&a, row("docs/b.md", "2026-11-27T00:00:01Z"))
			.await
			.unwrap();
		let snap = versions
			.create_snapshot("before", ACTOR, "2026-11-27T00:00:02Z")
			.await
			.unwrap();
		assert!(versions
			.create_snapshot("before", ACTOR, "2026-11-27T00:00:02Z")
			.await
			.is_err());

		let template = row("", "2026-11-27T00:00:03Z");
		assert_eq!(versions.moved("docs", "notes", &template).await.unwrap(), 2);
		assert_eq!(versions.removed("notes/b.md", &template).await.unwrap(), 1);
		let now: Vec<String> = versions
			.manifest(None, None)
			.await
			.unwrap()
			.into_iter()
			.map(|v| v.path)
			.collect();
		assert_eq!(now, ["notes/a.md"]);
		let then: Vec<String> = versions
			.manifest(Some(snap.manifest_id), None)
			.await
			.unwrap()
			.into_iter()
			.map(|v| v.path)
			.collect();
		assert_eq!(then, ["docs/a.md", "docs/b.md"]);
		assert_eq!(
			versions
				.history(Some("docs"), None, 10)
				.await
				.unwrap()
				.len(),
			4,
			"two captures, and a delete for each move"
		);
	}

	#[tokio::test]
	async fn retention_keeps_the_current_version_and_what_snapshots_pin() {
		let (dir, versions) = open().await;
		let file = dir.path().join("f");
		let capture = |text: &'static str, at: &'static str| {
			let (file, versions) = (file.clone(), versions.clone());
			async move {
				std::fs::write(&file, text).unwrap();
				versions
					.capture(&file, row("f.txt", at))
					.await
					.unwrap()
					.unwrap()
					.0
			}
		};
		let old = capture("one", "2026-09-01T00:00:00Z").await;
		let pinned = capture("two", "2026-09-02T00:00:00Z").await;
		let snap = versions
			.create_snapshot("keep", ACTOR, "2026-09-02T00:00:01Z")
			.await
			.unwrap();
		let also_old = capture("three", "2026-09-03T00:00:00Z").await;
		let current = capture("four", "2026-09-04T00:00:00Z").await;

		let pruned = versions.prune("2026-11-01T00:00:00Z").await.unwrap();
		assert_eq!(pruned.versions, 2);
		assert_eq!(pruned.blobs, 2);
		assert_eq!(pruned.chunks, 2);
		let left: Vec<i64> = versions
			.history(None, None, 10)
			.await
			.unwrap()
			.into_iter()
			.map(|v| v.id)
			.collect();
		assert_eq!(left, [current, pinned]);
		assert!(versions.version(old).await.unwrap().is_none());
		assert!(versions.version(also_old).await.unwrap().is_none());
		let out = dir.path().join("out");
		let kept = versions.version(pinned).await.unwrap().unwrap();
		versions
			.read_blob_to(kept.blob_hash.as_deref().unwrap(), &out)
			.await
			.unwrap();
		assert_eq!(std::fs::read_to_string(&out).unwrap(), "two");

		assert!(versions.delete_snapshot(snap.id).await.unwrap());
		let pruned = versions.prune("2026-11-01T00:00:00Z").await.unwrap();
		assert_eq!(pruned.versions, 1, "unpinned, it ages out");
		assert_eq!(
			versions
				.history(None, None, 10)
				.await
				.unwrap()
				.into_iter()
				.map(|v| v.id)
				.collect::<Vec<_>>(),
			[current]
		);
	}

	#[tokio::test]
	async fn a_damaged_chunk_is_refused_on_read() {
		let (dir, versions) = open().await;
		let file = dir.path().join("f");
		std::fs::write(&file, "precious").unwrap();
		let (_, stored) = versions
			.capture(&file, row("f", "2026-11-27T00:00:00Z"))
			.await
			.unwrap()
			.unwrap();
		let chunk = versions.chunks_of(&stored.hash).await.unwrap()[0];
		std::fs::write(versions.chunk_path(&chunk), "tampered").unwrap();
		assert!(versions
			.read_blob_to(&stored.hash, &dir.path().join("out"))
			.await
			.is_err());
	}
}
