//! Outputs for library backup, verification and restore.

use crate::infra::action::output::ActionOutputTrait;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LibraryBackupOutput {
	pub library_id: Uuid,
	/// The backup directory or archive written.
	pub destination: PathBuf,
	/// The manifest inside it; for an archive, its path once unpacked.
	pub manifest_path: PathBuf,
	pub files: u32,
	pub bytes: u64,
	pub duration_ms: u64,
	/// Sources the library registers but this device holds no store for.
	pub sources_without_store: Vec<Uuid>,
}

impl ActionOutputTrait for LibraryBackupOutput {
	fn to_json(&self) -> serde_json::Value {
		serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
	}

	fn display_message(&self) -> String {
		format!(
			"Backed up library {} to {} ({} files, {} bytes, {} ms)",
			self.library_id,
			self.destination.display(),
			self.files,
			self.bytes,
			self.duration_ms
		)
	}

	fn output_type(&self) -> &'static str {
		"library.backup.output"
	}
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LibraryBackupVerifyOutput {
	pub library_id: Uuid,
	pub library_name: String,
	pub created_at: chrono::DateTime<chrono::Utc>,
	pub build_sha: String,
	pub files: u32,
	pub bytes: u64,
	pub sources: u32,
	/// Files that are missing or differ from the manifest, with the reason.
	pub failures: Vec<VerifyFailure>,
	/// Migrations the backup applied that this build does not know; a
	/// restore here would be refused.
	pub unknown_migrations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct VerifyFailure {
	pub path: String,
	pub reason: String,
}

impl LibraryBackupVerifyOutput {
	pub fn ok(&self) -> bool {
		self.failures.is_empty()
	}
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LibraryRestoreOutput {
	pub library_id: Uuid,
	pub library_name: String,
	pub path: PathBuf,
	pub files: u32,
	pub bytes: u64,
	pub sources: u32,
	/// Where the state the restore replaced was moved, so a bad restore can
	/// be undone by hand. Absent for a new library.
	pub replaced_state: Option<PathBuf>,
}

impl ActionOutputTrait for LibraryRestoreOutput {
	fn to_json(&self) -> serde_json::Value {
		serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
	}

	fn display_message(&self) -> String {
		format!(
			"Restored library '{}' ({}) to {} ({} files, {} sources)",
			self.library_name,
			self.library_id,
			self.path.display(),
			self.files,
			self.sources
		)
	}

	fn output_type(&self) -> &'static str {
		"library.restore.output"
	}
}
