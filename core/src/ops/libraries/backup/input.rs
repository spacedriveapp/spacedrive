//! Inputs for library backup, verification and restore.

use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;
use uuid::Uuid;

/// Back up one library and the source stores this device holds for it.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LibraryBackupInput {
	pub library_id: Uuid,
	/// A directory that does not exist yet, or a `.tar.zst` file to write.
	pub destination: PathBuf,
	/// Thumbnail sidecars are derived from file bytes but expensive to bake
	/// again, so they come along by default.
	#[serde(default = "default_true")]
	pub include_sidecars: bool,
	/// Replicas of other devices' sources are fetched again on demand, so
	/// they stay out unless asked for.
	#[serde(default)]
	pub include_replicas: bool,
}

fn default_true() -> bool {
	true
}

/// Check a backup against its manifest without restoring it.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LibraryBackupVerifyInput {
	/// A backup directory, its `manifest.json`, or a `.tar.zst` archive.
	pub source: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum RestoreMode {
	/// Replace an existing library's state with the backup's.
	Replace,
	/// Create a library from the backup. Refuses to overwrite anything.
	New,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LibraryRestoreInput {
	/// A backup directory, its `manifest.json`, or a `.tar.zst` archive.
	pub source: PathBuf,
	pub mode: RestoreMode,
	/// The library to replace, or the id the new library takes. Defaults to
	/// the id the backup was taken from.
	#[serde(default)]
	pub library_id: Option<Uuid>,
	/// Replace a library that other devices are members of. Their device
	/// rows and sync watermarks live in the library being replaced, so after
	/// a forced restore they disagree with what those peers believe was
	/// delivered.
	#[serde(default)]
	pub force: bool,
}
