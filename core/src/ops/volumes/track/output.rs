//! Volume track output

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct VolumeTrackOutput {
	/// UUID of the tracked volume
	pub volume_id: Uuid,

	/// Fingerprint of the volume
	pub fingerprint: String,

	/// Display name
	pub name: String,

	/// Whether the volume is currently online
	pub is_online: bool,
}
