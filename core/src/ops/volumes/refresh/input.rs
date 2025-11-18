//! Volume refresh input

use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct VolumeRefreshInput {
	/// Optional: Set to true to force recalculation even if recently calculated
	#[serde(default)]
	pub force: bool,
}

impl Default for VolumeRefreshInput {
	fn default() -> Self {
		Self { force: false }
	}
}

