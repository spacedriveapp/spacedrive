//! # Consistent copies of live SQLite files
//!
//! Every database in a data directory is copied with `VACUUM INTO` on a
//! fresh connection. The copy is one consistent transaction of the source
//! as of the moment it started, whatever the daemon's own pools are doing:
//! in WAL mode readers never block writers, so the daemon keeps writing
//! while the copy is taken, and the copy carries everything committed
//! before it began. Copying the file bytes would not give that; a WAL
//! database's committed state is split between the main file and the log
//! until a checkpoint.
//!
//! The copy is also compact and has no WAL of its own, which is what makes
//! its hash a stable identity for the manifest.

use sqlx::{sqlite::SqliteConnectOptions, ConnectOptions, Connection, SqliteConnection};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;
use uuid::Uuid;

/// Write a consistent snapshot of the database at `source` to `dest`.
///
/// `dest` must not exist: `VACUUM INTO` refuses to overwrite, which is the
/// behaviour a backup wants.
pub async fn sqlite_copy(source: &Path, dest: &Path) -> Result<(), String> {
	let dest_str = dest
		.to_str()
		.ok_or_else(|| format!("snapshot path is not valid UTF-8: {}", dest.display()))?;
	if let Some(parent) = dest.parent() {
		tokio::fs::create_dir_all(parent)
			.await
			.map_err(|e| format!("create {}: {e}", parent.display()))?;
	}

	// The daemon's pools hold the file in WAL mode already; this connection
	// joins them. A long busy timeout covers a writer mid-checkpoint.
	let mut conn = SqliteConnectOptions::new()
		.filename(source)
		.create_if_missing(false)
		.busy_timeout(Duration::from_secs(60))
		.disable_statement_logging()
		.connect()
		.await
		.map_err(|e| format!("open {}: {e}", source.display()))?;
	sqlx::query("VACUUM INTO ?")
		.bind(dest_str)
		.execute(&mut conn)
		.await
		.map_err(|e| format!("vacuum {} into {}: {e}", source.display(), dest.display()))?;
	conn.close()
		.await
		.map_err(|e| format!("close {}: {e}", source.display()))?;
	Ok(())
}

async fn open_read_only(path: &Path) -> Result<SqliteConnection, String> {
	SqliteConnectOptions::new()
		.filename(path)
		.read_only(true)
		.disable_statement_logging()
		.connect()
		.await
		.map_err(|e| format!("open {}: {e}", path.display()))
}

/// A store copy's identity, read from the copy itself.
pub async fn store_identity(copy: &Path) -> Result<super::manifest::StoreEntry, String> {
	let mut conn = open_read_only(copy).await?;
	let revision: Option<(Uuid, i64)> =
		sqlx::query_as("SELECT store_id, value FROM _revision WHERE id = 1")
			.fetch_optional(&mut conn)
			.await
			.map_err(|e| format!("read revision of {}: {e}", copy.display()))?;
	let schema_hash: Option<String> =
		sqlx::query_scalar("SELECT schema_hash FROM _schema WHERE id = 1")
			.fetch_optional(&mut conn)
			.await
			.map_err(|e| format!("read schema of {}: {e}", copy.display()))?;
	let _ = conn.close().await;
	let (store_id, revision) = revision.unwrap_or((Uuid::nil(), 0));
	Ok(super::manifest::StoreEntry {
		store_id,
		revision,
		schema_hash: schema_hash.unwrap_or_default(),
	})
}

/// Migration names applied to a library database copy, in application order.
pub async fn applied_migrations(copy: &Path) -> Result<Vec<String>, String> {
	let mut conn = open_read_only(copy).await?;
	let names: Vec<String> =
		sqlx::query_scalar("SELECT version FROM seaql_migrations ORDER BY applied_at, version")
			.fetch_all(&mut conn)
			.await
			.map_err(|e| format!("read migrations of {}: {e}", copy.display()))?;
	let _ = conn.close().await;
	Ok(names)
}

/// Devices registered in a library database copy other than `this_device`.
pub async fn other_member_devices(copy: &Path, this_device: Uuid) -> Result<Vec<String>, String> {
	let mut conn = open_read_only(copy).await?;
	let rows: Vec<(Uuid, String)> = sqlx::query_as("SELECT uuid, name FROM devices")
		.fetch_all(&mut conn)
		.await
		.map_err(|e| format!("read devices of {}: {e}", copy.display()))?;
	let _ = conn.close().await;
	Ok(rows
		.into_iter()
		.filter(|(uuid, _)| *uuid != this_device)
		.map(|(uuid, name)| format!("{name} ({uuid})"))
		.collect())
}

/// Length and blake3 hash of a file, streamed off the blocking pool.
pub async fn hash_file(path: &Path) -> Result<(u64, String), String> {
	let path = path.to_path_buf();
	tokio::task::spawn_blocking(move || {
		let mut file =
			std::fs::File::open(&path).map_err(|e| format!("open {}: {e}", path.display()))?;
		let mut hasher = blake3::Hasher::new();
		let mut buffer = vec![0u8; 1 << 20];
		let mut total = 0u64;
		loop {
			let read = file
				.read(&mut buffer)
				.map_err(|e| format!("read {}: {e}", path.display()))?;
			if read == 0 {
				break;
			}
			hasher.update(&buffer[..read]);
			total += read as u64;
		}
		Ok((total, hasher.finalize().to_hex().to_string()))
	})
	.await
	.map_err(|e| format!("hashing task failed: {e}"))?
}

/// Whether a path names a `.tar.zst` archive rather than a directory.
pub fn is_archive(path: &Path) -> bool {
	path.to_string_lossy().ends_with(".tar.zst")
}

/// Pack a staged backup directory into a zstd-compressed tar at `archive`.
pub async fn pack(dir: &Path, archive: &Path) -> Result<(), String> {
	let dir = dir.to_path_buf();
	let archive = archive.to_path_buf();
	tokio::task::spawn_blocking(move || -> Result<(), String> {
		let file = std::fs::File::create(&archive)
			.map_err(|e| format!("create {}: {e}", archive.display()))?;
		let encoder =
			zstd::stream::write::Encoder::new(file, 3).map_err(|e| format!("zstd encoder: {e}"))?;
		let mut builder = tar::Builder::new(encoder);
		builder.follow_symlinks(false);
		builder
			.append_dir_all(".", &dir)
			.map_err(|e| format!("pack {}: {e}", dir.display()))?;
		let encoder = builder
			.into_inner()
			.map_err(|e| format!("finish tar: {e}"))?;
		encoder
			.finish()
			.map_err(|e| format!("finish zstd: {e}"))?
			.sync_all()
			.map_err(|e| format!("sync {}: {e}", archive.display()))?;
		Ok(())
	})
	.await
	.map_err(|e| format!("packing task failed: {e}"))?
}

/// Unpack a `.tar.zst` backup into `dir`, refusing entries that would
/// escape it.
pub async fn unpack(archive: &Path, dir: &Path) -> Result<(), String> {
	let archive = archive.to_path_buf();
	let dir = dir.to_path_buf();
	tokio::task::spawn_blocking(move || -> Result<(), String> {
		std::fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
		let file = std::fs::File::open(&archive)
			.map_err(|e| format!("open {}: {e}", archive.display()))?;
		let decoder =
			zstd::stream::read::Decoder::new(file).map_err(|e| format!("zstd decoder: {e}"))?;
		let mut tar = tar::Archive::new(decoder);
		tar.set_overwrite(false);
		for entry in tar
			.entries()
			.map_err(|e| format!("read {}: {e}", archive.display()))?
		{
			let mut entry = entry.map_err(|e| format!("read {}: {e}", archive.display()))?;
			let path = entry
				.path()
				.map_err(|e| format!("entry path: {e}"))?
				.into_owned();
			if path
				.components()
				.any(|component| matches!(component, std::path::Component::ParentDir))
			{
				return Err(format!(
					"archive entry escapes the backup: {}",
					path.display()
				));
			}
			entry
				.unpack_in(&dir)
				.map_err(|e| format!("unpack {}: {e}", path.display()))?;
		}
		Ok(())
	})
	.await
	.map_err(|e| format!("unpacking task failed: {e}"))?
}

/// Copy a plain file, creating the parent directory.
pub async fn copy_file(from: &Path, to: &Path) -> Result<u64, String> {
	if let Some(parent) = to.parent() {
		tokio::fs::create_dir_all(parent)
			.await
			.map_err(|e| format!("create {}: {e}", parent.display()))?;
	}
	tokio::fs::copy(from, to)
		.await
		.map_err(|e| format!("copy {} to {}: {e}", from.display(), to.display()))
}

/// Rename, falling back to copy-and-remove across filesystems.
pub async fn move_path(from: &Path, to: &Path) -> Result<(), String> {
	if let Some(parent) = to.parent() {
		tokio::fs::create_dir_all(parent)
			.await
			.map_err(|e| format!("create {}: {e}", parent.display()))?;
	}
	if tokio::fs::rename(from, to).await.is_ok() {
		return Ok(());
	}
	if from.is_dir() {
		copy_dir(from, to).await?;
		tokio::fs::remove_dir_all(from)
			.await
			.map_err(|e| format!("remove {}: {e}", from.display()))
	} else {
		copy_file(from, to).await?;
		tokio::fs::remove_file(from)
			.await
			.map_err(|e| format!("remove {}: {e}", from.display()))
	}
}

pub async fn copy_dir(from: &Path, to: &Path) -> Result<(), String> {
	let from = from.to_path_buf();
	let to = to.to_path_buf();
	tokio::task::spawn_blocking(move || copy_dir_blocking(&from, &to))
		.await
		.map_err(|e| format!("copy task failed: {e}"))?
}

fn copy_dir_blocking(from: &Path, to: &Path) -> Result<(), String> {
	std::fs::create_dir_all(to).map_err(|e| format!("create {}: {e}", to.display()))?;
	for entry in std::fs::read_dir(from).map_err(|e| format!("read {}: {e}", from.display()))? {
		let entry = entry.map_err(|e| format!("read {}: {e}", from.display()))?;
		let target = to.join(entry.file_name());
		if entry.path().is_dir() {
			copy_dir_blocking(&entry.path(), &target)?;
		} else {
			std::fs::copy(entry.path(), &target)
				.map_err(|e| format!("copy {}: {e}", entry.path().display()))?;
		}
	}
	Ok(())
}

/// Files under `dir`, recursively, as paths relative to it, sorted.
pub fn files_under(dir: &Path) -> Result<Vec<PathBuf>, String> {
	fn walk(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
		for entry in std::fs::read_dir(dir).map_err(|e| format!("read {}: {e}", dir.display()))? {
			let entry = entry.map_err(|e| format!("read {}: {e}", dir.display()))?;
			let path = entry.path();
			if path.is_dir() {
				walk(root, &path, out)?;
			} else if let Ok(relative) = path.strip_prefix(root) {
				out.push(relative.to_path_buf());
			}
		}
		Ok(())
	}
	let mut out = Vec::new();
	walk(dir, dir, &mut out)?;
	out.sort();
	Ok(out)
}

/// Source ids registered in a library database file.
pub async fn source_ids_of(library_db: &Path) -> Result<Vec<Uuid>, String> {
	let mut conn = open_read_only(library_db).await?;
	let ids: Vec<Uuid> = sqlx::query_scalar("SELECT uuid FROM sources")
		.fetch_all(&mut conn)
		.await
		.map_err(|e| format!("read sources of {}: {e}", library_db.display()))?;
	let _ = conn.close().await;
	Ok(ids)
}
