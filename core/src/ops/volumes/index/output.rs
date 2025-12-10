//! Output types for volume indexing action

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

/// Output from volume indexing action
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct IndexVolumeOutput {
	/// UUID of the indexed volume
	pub volume_id: Uuid,
	/// Job ID for tracking progress
	pub job_id: Uuid,
	/// Total files found (if job completed)
	pub total_files: Option<u64>,
	/// Total directories found (if job completed)
	pub total_directories: Option<u64>,
	/// Success message
	pub message: String,
}
