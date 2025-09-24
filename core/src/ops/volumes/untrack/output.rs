//! Volume untrack operation output types

use crate::volume::VolumeFingerprint;
use serde::{Deserialize, Serialize};
use specta::Type;

/// Output from volume untrack operation
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct VolumeUntrackOutput {
	/// The fingerprint of the untracked volume
	pub fingerprint: VolumeFingerprint,
}

impl VolumeUntrackOutput {
	/// Create new volume untrack output
	pub fn new(fingerprint: VolumeFingerprint) -> Self {
		Self { fingerprint }
	}
}
