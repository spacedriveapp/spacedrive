use serde::{Deserialize, Serialize};

/// Input for ejecting a volume
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct VolumeEjectInput {
	/// Fingerprint of the volume to eject
	pub fingerprint: String,
}
