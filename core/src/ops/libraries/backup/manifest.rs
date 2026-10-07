//! # Backup manifest
//!
//! The manifest is what makes a backup checkable before it is trusted. Every
//! file in the backup is listed with its byte length and blake3 hash, so a
//! restore can prove the copy is the one that was written before it touches
//! a data directory, and `verify` can answer the same question without
//! restoring at all.
//!
//! The library's applied migrations are recorded by name because that is how
//! SeaORM tracks them. A build restores a backup only when it knows every
//! migration the backup has applied; a backup from a newer build is refused
//! instead of being opened and migrated downward.

use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Bump when the layout or the meaning of a manifest field changes.
pub const MANIFEST_FORMAT: u32 = 1;

pub const MANIFEST_FILE: &str = "manifest.json";

/// Everything a restore or a verification needs to know about a backup.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct BackupManifest {
	pub format: u32,
	pub created_at: chrono::DateTime<chrono::Utc>,
	/// The device the backup was taken on.
	pub device_id: Uuid,
	pub build_sha: String,
	pub core_version: String,
	pub library: LibraryEntry,
	pub include_sidecars: bool,
	pub include_replicas: bool,
	pub sources: Vec<SourceEntry>,
	/// Every file in the backup beside the manifest itself, with paths
	/// relative to the backup root and `/` as the separator.
	pub files: Vec<FileEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LibraryEntry {
	pub id: Uuid,
	pub name: String,
	/// Migration names the library database had applied, in order.
	pub applied_migrations: Vec<String>,
	pub has_sync_db: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SourceEntry {
	pub id: Uuid,
	pub name: String,
	pub data_type: String,
	pub root: Option<String>,
	/// Absent when this device holds no store for the source: the
	/// registration is in the library, the generation lives elsewhere.
	pub store: Option<StoreEntry>,
	pub has_sidecars: bool,
	/// Where the source keeps its store. A store placed on the source is
	/// archived from the drive, and a restore cannot put it back there, so
	/// the restore names it instead of counting it. Archives written before
	/// placement existed hold in-library stores.
	#[serde(default)]
	pub placement: crate::ops::indexing::sources::StorePlacement,
}

/// How a store's copy identifies itself; see `sd_store::revision`.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct StoreEntry {
	pub store_id: Uuid,
	pub revision: i64,
	pub schema_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct FileEntry {
	pub path: String,
	pub bytes: u64,
	pub blake3: String,
}

impl BackupManifest {
	pub async fn load(path: &Path) -> Result<Self, String> {
		let bytes = tokio::fs::read(path)
			.await
			.map_err(|e| format!("read {}: {e}", path.display()))?;
		let manifest: Self = serde_json::from_slice(&bytes)
			.map_err(|e| format!("{} is not a backup manifest: {e}", path.display()))?;
		if manifest.format > MANIFEST_FORMAT {
			return Err(format!(
				"backup manifest format {} is newer than this build understands ({})",
				manifest.format, MANIFEST_FORMAT
			));
		}
		Ok(manifest)
	}

	pub async fn save(&self, path: &Path) -> Result<(), String> {
		let json = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
		tokio::fs::write(path, json)
			.await
			.map_err(|e| format!("write {}: {e}", path.display()))
	}

	/// Where a listed file sits under a backup root.
	pub fn file_path(root: &Path, relative: &str) -> PathBuf {
		relative
			.split('/')
			.fold(root.to_path_buf(), |path, segment| path.join(segment))
	}

	pub fn total_bytes(&self) -> u64 {
		self.files.iter().map(|file| file.bytes).sum()
	}

	/// Check every listed file against its recorded length and hash.
	///
	/// Returns the files that differ or are missing, each with the reason,
	/// so a report can name them. Empty means the backup is intact.
	pub async fn verify(&self, root: &Path) -> Vec<(String, String)> {
		let mut failures = Vec::new();
		for entry in &self.files {
			let path = Self::file_path(root, &entry.path);
			match super::snapshot::hash_file(&path).await {
				Ok((bytes, hash)) if bytes == entry.bytes && hash == entry.blake3 => {}
				Ok((bytes, hash)) => failures.push((
					entry.path.clone(),
					format!(
						"expected {} bytes with blake3 {}, found {bytes} bytes with blake3 {hash}",
						entry.bytes, entry.blake3
					),
				)),
				Err(error) => failures.push((entry.path.clone(), error)),
			}
		}
		failures
	}

	/// Migrations the backup applied that this build has never heard of.
	pub fn unknown_migrations(&self) -> Vec<String> {
		use sea_orm_migration::MigratorTrait;
		let known: std::collections::HashSet<String> =
			crate::infra::db::migration::Migrator::migrations()
				.iter()
				.map(|migration| migration.name().to_string())
				.collect();
		self.library
			.applied_migrations
			.iter()
			.filter(|name| !known.contains(*name))
			.cloned()
			.collect()
	}
}
