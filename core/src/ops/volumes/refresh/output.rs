//! Volume refresh output

use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct VolumeRefreshOutput {
	/// Number of volumes that had their unique_bytes calculated
	pub volumes_refreshed: usize,
	/// Number of volumes that failed to refresh
	pub volumes_failed: usize,
}

impl VolumeRefreshOutput {
	pub fn new(volumes_refreshed: usize, volumes_failed: usize) -> Self {
		Self {
			volumes_refreshed,
			volumes_failed,
		}
	}
}

