//! Volume track operation output types

use crate::volume::VolumeFingerprint;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Output from volume track operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeTrackOutput {
	/// The fingerprint of the tracked volume
	pub fingerprint: VolumeFingerprint,

	/// The library ID where the volume was tracked
	pub library_id: Uuid,

	/// The display name of the tracked volume
	pub volume_name: String,
}

impl VolumeTrackOutput {
	/// Create new volume track output
	pub fn new(fingerprint: VolumeFingerprint, library_id: Uuid, volume_name: String) -> Self {
		Self {
			fingerprint,
			library_id,
			volume_name,
		}
	}
}
