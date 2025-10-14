//! Volume remove cloud operation output types

use crate::volume::VolumeFingerprint;
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct VolumeRemoveCloudOutput {
	pub fingerprint: VolumeFingerprint,
}

impl VolumeRemoveCloudOutput {
	pub fn new(fingerprint: VolumeFingerprint) -> Self {
		Self { fingerprint }
	}
}
