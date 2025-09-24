//! Volume track operation output types

use crate::volume::VolumeFingerprint;
use serde::{Deserialize, Serialize};
use specta::Type;

/// Output from volume track operation
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct VolumeTrackOutput {
	/// The fingerprint of the tracked volume
	pub fingerprint: VolumeFingerprint,

	/// The display name of the tracked volume
	pub volume_name: String,
}

impl VolumeTrackOutput {
	/// Create new volume track output
	pub fn new(fingerprint: VolumeFingerprint, volume_name: String) -> Self {
		Self {
			fingerprint,
			volume_name,
		}
	}
}
