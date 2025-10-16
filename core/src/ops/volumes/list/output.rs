//! Volume list output

use crate::volume::VolumeFingerprint;
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct VolumeItem {
	pub uuid: Uuid,
	pub name: String,
	pub fingerprint: VolumeFingerprint,
	pub volume_type: String,
	pub mount_point: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct VolumeListOutput {
	pub volumes: Vec<VolumeItem>,
}
