//! Volume track input

use crate::volume::VolumeFingerprint;
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct VolumeTrackInput {
	/// Fingerprint of the volume to track
	pub fingerprint: String,

	/// Optional custom display name
	pub display_name: Option<String>,
}







