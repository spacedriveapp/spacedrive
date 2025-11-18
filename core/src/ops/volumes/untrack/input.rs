//! Volume untrack input

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct VolumeUntrackInput {
	/// UUID of the volume to untrack
	pub volume_id: Uuid,
}


