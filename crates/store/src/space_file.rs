//! # Portable space file
//!
//! One SQLite file that carries a whole space to another library: the space
//! record, its change log, its versions and snapshots, and the chunks their
//! content needs. `sd space export` writes it and `sd space import` reads it.
//! An export with only the current files carries the newest version of each
//! file and no snapshots.
//!
//! The file is self-describing: `meta` names the format version and holds
//! the space record as JSON, and the other tables have the same columns as
//! a space's `events.db` and `versions.db`, plus `chunk`, which holds the
//! chunk bytes the version store keeps as files. Every chunk is checked
//! against its hash when it is read back.
//!
//! ## Example
//! ```rust,no_run
//! use std::path::Path;
//!
//! use sd_store::{space_file::SpaceFile, SpaceVersions};
//!
//! async fn round_trip(versions: &SpaceVersions, target: &SpaceVersions) -> sd_store::Result<()> {
//!     let out = Path::new("/tmp/project.sdspace");
//!     let file = SpaceFile::create(out).await?;
//!     file.put_versions(versions, false).await?;
//!     file.close().await;
//!
//!     let file = SpaceFile::open(out).await?;
//!     file.check().await?;
//!     file.restore_versions(target, &Default::default()).await?;
//!     Ok(())
//! }
//! ```

use std::{
	collections::{HashMap, HashSet},
	path::Path,
};

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::{Connection, SqlitePool};

use crate::{
	error::{Error, Result},
	space_events::SpaceEvent,
	space_versions::{SnapshotRow, SpaceVersions, VersionRow},
};

/// The format version this build writes and reads.
pub const SPACE_FILE_FORMAT: i64 = 1;

/// The file extension of an exported space.
pub const SPACE_FILE_EXTENSION: &str = "sdspace";

const SCHEMA: &str = r#"
CREATE TABLE meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
CREATE TABLE space_event (
    seq INTEGER PRIMARY KEY,
    space_id BLOB NOT NULL,
    at TEXT NOT NULL,
    actor_id BLOB NOT NULL,
    grant_id BLOB,
    origin TEXT NOT NULL,
    op TEXT NOT NULL,
    path TEXT NOT NULL,
    to_path TEXT,
    before_hash TEXT,
    after_hash TEXT,
    size INTEGER,
    undo_id BLOB,
    authority_epoch INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE version (
    id INTEGER PRIMARY KEY,
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
CREATE TABLE snapshot (
    id BLOB PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    manifest_id INTEGER NOT NULL,
    created_by BLOB NOT NULL,
    at TEXT NOT NULL
);
CREATE TABLE blob (
    hash TEXT PRIMARY KEY,
    size INTEGER NOT NULL,
    chunks BLOB NOT NULL
);
CREATE TABLE chunk (
    hash BLOB PRIMARY KEY,
    data BLOB NOT NULL
);
"#;

/// What an export or an import carried.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SpaceFileCounts {
	pub events: u64,
	pub versions: u64,
	pub snapshots: u64,
	pub blobs: u64,
	pub chunks: u64,
	pub chunk_bytes: u64,
}

/// A portable space file, open for writing an export or reading an import.
#[derive(Debug, Clone)]
pub struct SpaceFile {
	pool: SqlitePool,
}

impl SpaceFile {
	/// Create a new file at `path`. An existing file is refused, never
	/// replaced.
	pub async fn create(path: &Path) -> Result<Self> {
		if tokio::fs::symlink_metadata(path).await.is_ok() {
			return Err(Error::Other(format!("{} already exists", path.display())));
		}
		let pool = Self::connect(path, true).await?;
		sqlx::raw_sql(SCHEMA).execute(&pool).await?;
		sqlx::query("INSERT INTO meta (key, value) VALUES ('format', ?)")
			.bind(SPACE_FILE_FORMAT.to_string())
			.execute(&pool)
			.await?;
		Ok(Self { pool })
	}

	/// Open an existing file to read it. A file from a newer format is
	/// refused.
	pub async fn open(path: &Path) -> Result<Self> {
		let pool = Self::connect(path, false).await?;
		let file = Self { pool };
		let format = file
			.meta("format")
			.await?
			.and_then(|format| format.parse::<i64>().ok())
			.ok_or_else(|| Error::Other(format!("{} is not a space file", path.display())))?;
		if format > SPACE_FILE_FORMAT {
			return Err(Error::Other(format!(
				"{} was written by a newer Spacedrive (format {format})",
				path.display()
			)));
		}
		Ok(file)
	}

	async fn connect(path: &Path, create: bool) -> Result<SqlitePool> {
		let options = SqliteConnectOptions::new()
			.filename(path)
			.create_if_missing(create)
			.read_only(!create)
			.journal_mode(SqliteJournalMode::Delete);
		Ok(SqlitePoolOptions::new()
			.max_connections(1)
			.connect_with(options)
			.await?)
	}

	/// Close the file, so it is complete on disk.
	pub async fn close(self) {
		self.pool.close().await;
	}

	pub async fn set_meta(&self, key: &str, value: &str) -> Result<()> {
		sqlx::query("INSERT INTO meta (key, value) VALUES (?, ?) ON CONFLICT (key) DO UPDATE SET value = excluded.value")
			.bind(key)
			.bind(value)
			.execute(&self.pool)
			.await?;
		Ok(())
	}

	pub async fn meta(&self, key: &str) -> Result<Option<String>> {
		let value = sqlx::query_scalar("SELECT value FROM meta WHERE key = ?")
			.bind(key)
			.fetch_optional(&self.pool)
			.await;
		match value {
			Ok(value) => Ok(value),
			Err(sqlx::Error::Database(db)) if db.message().contains("no such table") => Ok(None),
			Err(error) => Err(error.into()),
		}
	}

	/// Write the change log.
	pub async fn put_events(&self, events: &[SpaceEvent]) -> Result<()> {
		let mut conn = self.pool.acquire().await?;
		let mut tx = conn.begin().await?;
		for event in events {
			sqlx::query(
				"INSERT INTO space_event
					(seq, space_id, at, actor_id, grant_id, origin, op, path, to_path,
					 before_hash, after_hash, size, undo_id, authority_epoch)
				 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
			)
			.bind(event.seq)
			.bind(event.space_id)
			.bind(&event.at)
			.bind(event.actor_id)
			.bind(event.grant_id)
			.bind(&event.origin)
			.bind(&event.op)
			.bind(&event.path)
			.bind(&event.to_path)
			.bind(&event.before_hash)
			.bind(&event.after_hash)
			.bind(event.size)
			.bind(event.undo_id)
			.bind(event.authority_epoch)
			.execute(&mut *tx)
			.await?;
		}
		tx.commit().await?;
		Ok(())
	}

	/// The change log, oldest first.
	pub async fn events(&self) -> Result<Vec<SpaceEvent>> {
		Ok(sqlx::query_as(
			"SELECT seq, space_id, at, actor_id, grant_id, origin, op, path, to_path,
				before_hash, after_hash, size, undo_id, authority_epoch
			 FROM space_event ORDER BY seq",
		)
		.fetch_all(&self.pool)
		.await?)
	}

	/// Copy a space's versions in: every version and snapshot, or with
	/// `current_only` the newest version of each file and no snapshots, and
	/// the blobs and chunks those versions reach.
	pub async fn put_versions(
		&self,
		versions: &SpaceVersions,
		current_only: bool,
	) -> Result<SpaceFileCounts> {
		let mut counts = SpaceFileCounts::default();
		// No prune runs until the copy is done, and the snapshots are read
		// before the history, so every row a snapshot names is in it.
		let _no_prune = versions.hold_prunes().await;
		let snapshots = if current_only {
			Vec::new()
		} else {
			versions.snapshots().await?
		};
		let rows = if current_only {
			versions.manifest(None, None).await?
		} else {
			versions.history(None, None, u32::MAX).await?
		};
		let mut conn = self.pool.acquire().await?;
		let mut tx = conn.begin().await?;
		let mut blobs = HashSet::new();
		for row in rows.iter().rev() {
			insert_version(&mut tx, row).await?;
			counts.versions += 1;
			if let Some(hash) = &row.blob_hash {
				blobs.insert(hash.clone());
			}
		}
		for snapshot in &snapshots {
			insert_snapshot(&mut tx, snapshot).await?;
			counts.snapshots += 1;
		}
		let mut written = HashSet::new();
		for hash in blobs {
			let chunks = versions.chunks_of(&hash).await?;
			let size: i64 = sqlx::query_scalar("SELECT size FROM blob WHERE hash = ?")
				.bind(&hash)
				.fetch_one(versions.pool())
				.await?;
			let list: Vec<u8> = chunks.iter().flatten().copied().collect();
			sqlx::query("INSERT INTO blob (hash, size, chunks) VALUES (?, ?, ?)")
				.bind(&hash)
				.bind(size)
				.bind(list)
				.execute(&mut *tx)
				.await?;
			counts.blobs += 1;
			let mut whole = blake3::Hasher::new();
			for chunk in chunks {
				let data = tokio::fs::read(versions.chunk_path(&chunk)).await?;
				if blake3::hash(&data).as_bytes() != &chunk {
					return Err(Error::Other(format!("a stored chunk of {hash} is damaged")));
				}
				whole.update(&data);
				if !written.insert(chunk) {
					continue;
				}
				counts.chunk_bytes += data.len() as u64;
				sqlx::query("INSERT INTO chunk (hash, data) VALUES (?, ?)")
					.bind(chunk.as_slice())
					.bind(data)
					.execute(&mut *tx)
					.await?;
				counts.chunks += 1;
			}
			if whole.finalize().to_hex().as_str() != hash {
				return Err(Error::Other(format!(
					"stored content {hash} does not match its hash"
				)));
			}
		}
		tx.commit().await?;
		Ok(counts)
	}

	/// The newest version of each file the file carries, by path.
	pub async fn current(&self) -> Result<Vec<VersionRow>> {
		Ok(sqlx::query_as(
			"SELECT id, path, blob_hash, size, mode, mtime, at, actor_id, origin, event_seq
			 FROM version WHERE id IN (SELECT MAX(id) FROM version GROUP BY path)
				AND blob_hash IS NOT NULL ORDER BY path",
		)
		.fetch_all(&self.pool)
		.await?)
	}

	/// One chunk's bytes, checked against its hash.
	async fn chunk(&self, hash: &[u8]) -> Result<Vec<u8>> {
		let data: Vec<u8> = sqlx::query_scalar("SELECT data FROM chunk WHERE hash = ?")
			.bind(hash)
			.fetch_optional(&self.pool)
			.await?
			.ok_or_else(|| Error::Other("the file is missing a chunk".to_string()))?;
		if blake3::hash(&data).as_bytes().as_slice() != hash {
			return Err(Error::Other("a chunk in the file is damaged".to_string()));
		}
		Ok(data)
	}

	/// Check the whole file before anything is imported from it: every path
	/// it names stays inside a space, every version's content is present,
	/// and every blob rebuilds from its chunks to its hash. Reads one chunk
	/// at a time.
	pub async fn check(&self) -> Result<()> {
		let paths: Vec<String> = sqlx::query_scalar(
			"SELECT path FROM version UNION SELECT path FROM space_event
			 UNION SELECT to_path FROM space_event WHERE to_path IS NOT NULL",
		)
		.fetch_all(&self.pool)
		.await?;
		if let Some(bad) = paths.iter().find(|path| !is_plain_relative(path)) {
			return Err(Error::Other(format!(
				"the file names a path outside its space: {bad}"
			)));
		}
		let missing: Option<String> = sqlx::query_scalar(
			"SELECT blob_hash FROM version WHERE blob_hash IS NOT NULL
			 AND blob_hash NOT IN (SELECT hash FROM blob) LIMIT 1",
		)
		.fetch_optional(&self.pool)
		.await?;
		if let Some(hash) = missing {
			return Err(Error::Other(format!("the file is missing content {hash}")));
		}
		let blobs: Vec<(String, Vec<u8>)> = sqlx::query_as("SELECT hash, chunks FROM blob")
			.fetch_all(&self.pool)
			.await?;
		for (hash, list) in blobs {
			let mut whole = blake3::Hasher::new();
			for chunk in list.chunks_exact(blake3::OUT_LEN) {
				whole.update(&self.chunk(chunk).await?);
			}
			if list.len() % blake3::OUT_LEN != 0 || whole.finalize().to_hex().as_str() != hash {
				return Err(Error::Other(format!(
					"content {hash} in the file does not match its hash"
				)));
			}
		}
		Ok(())
	}

	/// Write the content `hash` to `dest`, checking each chunk and the whole.
	pub async fn read_blob_to(&self, hash: &str, dest: &Path) -> Result<u64> {
		use tokio::io::AsyncWriteExt;
		let list: Vec<u8> = sqlx::query_scalar("SELECT chunks FROM blob WHERE hash = ?")
			.bind(hash)
			.fetch_optional(&self.pool)
			.await?
			.ok_or_else(|| Error::Other(format!("the file carries no content {hash}")))?;
		let mut out = tokio::fs::File::create(dest).await?;
		let mut whole = blake3::Hasher::new();
		let mut size = 0u64;
		for chunk in list.chunks_exact(blake3::OUT_LEN) {
			let data = self.chunk(chunk).await?;
			whole.update(&data);
			out.write_all(&data).await?;
			size += data.len() as u64;
		}
		out.flush().await?;
		if whole.finalize().to_hex().as_str() != hash {
			return Err(Error::Other(format!(
				"content {hash} does not match its hash"
			)));
		}
		Ok(size)
	}

	/// Copy the versions, snapshots, blobs and chunks into a space's empty
	/// version store, keeping version ids so snapshots still name the right
	/// manifests. `seqs` maps the file's change-log seqs to the ones the
	/// imported log gave them; a version whose change is not in it keeps none.
	pub async fn restore_versions(
		&self,
		versions: &SpaceVersions,
		seqs: &HashMap<i64, i64>,
	) -> Result<SpaceFileCounts> {
		if versions.head().await? != 0 {
			return Err(Error::Other("the space already has versions".to_string()));
		}
		let mut counts = SpaceFileCounts::default();
		// One chunk in memory at a time, whatever the size of the file.
		let hashes: Vec<Vec<u8>> = sqlx::query_scalar("SELECT hash FROM chunk")
			.fetch_all(&self.pool)
			.await?;
		for hash in hashes {
			let data = self.chunk(&hash).await?;
			let hash: [u8; blake3::OUT_LEN] = hash
				.as_slice()
				.try_into()
				.map_err(|_| Error::Other("a chunk has a malformed hash".to_string()))?;
			counts.chunk_bytes += data.len() as u64;
			versions.put_chunk(&hash, &data).await?;
			counts.chunks += 1;
		}
		let blobs: Vec<(String, i64, Vec<u8>)> =
			sqlx::query_as("SELECT hash, size, chunks FROM blob")
				.fetch_all(&self.pool)
				.await?;
		let rows: Vec<VersionRow> = sqlx::query_as(
			"SELECT id, path, blob_hash, size, mode, mtime, at, actor_id, origin, event_seq
			 FROM version ORDER BY id",
		)
		.fetch_all(&self.pool)
		.await?;
		let snapshots: Vec<SnapshotRow> =
			sqlx::query_as("SELECT id, name, manifest_id, created_by, at FROM snapshot")
				.fetch_all(&self.pool)
				.await?;
		let mut conn = versions.pool().acquire().await?;
		let mut tx = conn.begin().await?;
		for (hash, size, list) in &blobs {
			sqlx::query("INSERT OR IGNORE INTO blob (hash, size, chunks) VALUES (?, ?, ?)")
				.bind(hash)
				.bind(size)
				.bind(list)
				.execute(&mut *tx)
				.await?;
			counts.blobs += 1;
		}
		for row in rows {
			let row = VersionRow {
				event_seq: row.event_seq.and_then(|seq| seqs.get(&seq).copied()),
				..row
			};
			insert_version(&mut tx, &row).await?;
			counts.versions += 1;
		}
		for snapshot in &snapshots {
			insert_snapshot(&mut tx, snapshot).await?;
			counts.snapshots += 1;
		}
		tx.commit().await?;
		Ok(counts)
	}
}

/// A path from a space file is used to write inside a folder, so it must be
/// relative and plain: no root, no `..`, no empty or `.` parts.
pub fn is_plain_relative(path: &str) -> bool {
	// A colon or a backslash could name a drive or a parent on Windows, so
	// neither is accepted anywhere, on any platform.
	!path.is_empty()
		&& !path.starts_with('/')
		&& !path.contains(['\\', ':', '\0'])
		&& path
			.split('/')
			.all(|part| !part.is_empty() && part != "." && part != "..")
}

async fn insert_version(conn: &mut sqlx::SqliteConnection, row: &VersionRow) -> Result<()> {
	sqlx::query(
		"INSERT INTO version (id, path, blob_hash, size, mode, mtime, at, actor_id, origin, event_seq)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
	)
	.bind(row.id)
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
	.await?;
	Ok(())
}

async fn insert_snapshot(conn: &mut sqlx::SqliteConnection, row: &SnapshotRow) -> Result<()> {
	sqlx::query(
		"INSERT INTO snapshot (id, name, manifest_id, created_by, at) VALUES (?, ?, ?, ?, ?)",
	)
	.bind(row.id)
	.bind(&row.name)
	.bind(row.manifest_id)
	.bind(row.created_by)
	.bind(&row.at)
	.execute(&mut *conn)
	.await?;
	Ok(())
}

#[cfg(test)]
mod tests {
	use uuid::Uuid;

	use super::*;

	#[tokio::test]
	async fn a_space_round_trips_through_a_file() {
		let dir = tempfile::tempdir().unwrap();
		let source = SpaceVersions::open(&dir.path().join("a")).await.unwrap();
		let file = dir.path().join("f");
		let row = |path: &str, at: &str| VersionRow {
			path: path.into(),
			..VersionRow::attributed(at, Uuid::from_u128(3), "agent_write", None)
		};
		std::fs::write(&file, "one").unwrap();
		source
			.capture(&file, row("notes.md", "2026-11-27T00:00:00Z"))
			.await
			.unwrap();
		source
			.create_snapshot("first", Uuid::from_u128(3), "2026-11-27T00:00:01Z")
			.await
			.unwrap();
		std::fs::write(&file, "two").unwrap();
		source
			.capture(&file, row("notes.md", "2026-11-27T00:00:02Z"))
			.await
			.unwrap();

		for current_only in [false, true] {
			let out = dir.path().join(format!("export-{current_only}.sdspace"));
			let export = SpaceFile::create(&out).await.unwrap();
			let written = export.put_versions(&source, current_only).await.unwrap();
			export.close().await;
			assert!(
				SpaceFile::create(&out).await.is_err(),
				"never replaces a file"
			);
			assert_eq!(written.versions, if current_only { 1 } else { 2 });

			let import = SpaceFile::open(&out).await.unwrap();
			let current = import.current().await.unwrap();
			assert_eq!(current.len(), 1);
			let back = dir.path().join("back");
			import
				.read_blob_to(current[0].blob_hash.as_deref().unwrap(), &back)
				.await
				.unwrap();
			assert_eq!(std::fs::read_to_string(&back).unwrap(), "two");

			let target = SpaceVersions::open(&dir.path().join(format!("b-{current_only}")))
				.await
				.unwrap();
			let read = import
				.restore_versions(&target, &HashMap::new())
				.await
				.unwrap();
			assert_eq!(read.versions, written.versions);
			let snapshots = target.snapshots().await.unwrap();
			assert_eq!(snapshots.len(), if current_only { 0 } else { 1 });
			if let Some(first) = snapshots.first() {
				let pinned = target
					.manifest(Some(first.manifest_id), None)
					.await
					.unwrap();
				target
					.read_blob_to(pinned[0].blob_hash.as_deref().unwrap(), &back)
					.await
					.unwrap();
				assert_eq!(std::fs::read_to_string(&back).unwrap(), "one");
			}
		}
	}

	#[tokio::test]
	async fn a_damaged_or_escaping_file_fails_its_check() {
		let dir = tempfile::tempdir().unwrap();
		let source = SpaceVersions::open(&dir.path().join("a")).await.unwrap();
		let file = dir.path().join("f");
		std::fs::write(&file, "one").unwrap();
		source
			.capture(
				&file,
				VersionRow {
					path: "notes.md".into(),
					..VersionRow::attributed(
						"2026-11-27T00:00:00Z",
						Uuid::from_u128(3),
						"agent_write",
						None,
					)
				},
			)
			.await
			.unwrap();
		for (name, tamper) in [
			("damaged", "UPDATE chunk SET data = x'00'"),
			("escaping", "UPDATE version SET path = 'C:/Users/x.txt'"),
			("missing", "DELETE FROM blob"),
		] {
			let out = dir.path().join(format!("{name}.sdspace"));
			let export = SpaceFile::create(&out).await.unwrap();
			export.put_versions(&source, false).await.unwrap();
			sqlx::query(tamper).execute(&export.pool).await.unwrap();
			export.close().await;
			let import = SpaceFile::open(&out).await.unwrap();
			assert!(import.check().await.is_err(), "{name}");
		}
	}

	#[test]
	fn only_plain_relative_paths_are_accepted() {
		assert!(is_plain_relative("a/b.md"));
		for bad in [
			"",
			"/etc/passwd",
			"../x",
			"a/../../x",
			"a//b",
			"./a",
			"a\\..\\b",
			"C:/Users/x.txt",
			"C:x.txt",
		] {
			assert!(!is_plain_relative(bad), "{bad}");
		}
	}
}
