use serde::{Deserialize, Serialize};

/// Output from volume eject operation
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct VolumeEjectOutput {
	/// The fingerprint of the ejected volume
	pub fingerprint: String,
	/// Whether the eject was successful
	pub success: bool,
	/// Optional message (error or success details)
	pub message: Option<String>,
}
