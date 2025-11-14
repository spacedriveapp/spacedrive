//! Volume list output

use crate::volume::VolumeFingerprint;
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct VolumeItem {
	pub id: Uuid,
	pub name: String,
	pub fingerprint: VolumeFingerprint,
	pub volume_type: String,
	pub mount_point: Option<String>,
	/// Whether this volume is currently tracked in the library
	pub is_tracked: bool,
	/// Whether this volume is currently online/mounted
	pub is_online: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct VolumeListOutput {
	pub volumes: Vec<VolumeItem>,
}
