//! Volume untrack output

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct VolumeUntrackOutput {
	/// UUID of the untracked volume
	pub volume_id: Uuid,

	/// Whether the operation was successful
	pub success: bool,
}
