//! Input for undoing a job

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

/// Reverse what a job did, from its journal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct FileUndoInput {
	pub job: Uuid,
	/// The effects to reverse, by sequence; every effect when absent. The
	/// trash view restores one item this way.
	#[serde(default)]
	pub effects: Option<Vec<i64>>,
}
