//! Volume untrack operation output types

use crate::volume::VolumeFingerprint;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Output from volume untrack operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeUntrackOutput {
	/// The fingerprint of the untracked volume
	pub fingerprint: VolumeFingerprint,

	/// The library ID from which the volume was untracked
	pub library_id: Uuid,
}

impl VolumeUntrackOutput {
	/// Create new volume untrack output
	pub fn new(fingerprint: VolumeFingerprint, library_id: Uuid) -> Self {
		Self {
			fingerprint,
			library_id,
		}
	}
}

