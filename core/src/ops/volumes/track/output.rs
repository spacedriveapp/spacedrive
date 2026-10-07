//! Volume track output

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use crate::ops::sources::track::TrackSourceOutput;

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

	/// The source set up over the whole drive, with the settings it was
	/// saved with and where its catalog lives. Absent when the volume is
	/// offline, since a source needs a mount point to walk.
	pub source: Option<TrackSourceOutput>,
}
