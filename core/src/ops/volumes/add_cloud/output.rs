//! Volume add cloud operation output types

use crate::volume::{backend::CloudServiceType, VolumeFingerprint};
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct VolumeAddCloudOutput {
	pub fingerprint: VolumeFingerprint,
	pub volume_name: String,
	pub service: CloudServiceType,
}

impl VolumeAddCloudOutput {
	pub fn new(
		fingerprint: VolumeFingerprint,
		volume_name: String,
		service: CloudServiceType,
	) -> Self {
		Self {
			fingerprint,
			volume_name,
			service,
		}
	}
}
