//! Strategy router for selecting the optimal delete method

use super::strategy::{DeleteStrategy, LocalDeleteStrategy, RemoteDeleteStrategy};
use crate::{domain::addressing::SdPath, volume::VolumeManager};

pub struct DeleteStrategyRouter;

impl DeleteStrategyRouter {
	/// Select optimal delete strategy based on path locations
	pub async fn select_strategy(
		paths: &[SdPath],
		_volume_manager: Option<&VolumeManager>,
	) -> Box<dyn DeleteStrategy> {
		// Check if all paths are local
		let all_local = paths.iter().all(|p| p.is_local());

		if all_local {
			Box::new(LocalDeleteStrategy)
		} else {
			// At least one remote path - use remote strategy
			Box::new(RemoteDeleteStrategy)
		}
	}

	/// Describe the strategy that will be used
	pub async fn describe_strategy(paths: &[SdPath]) -> String {
		let local_count = paths.iter().filter(|p| p.is_local()).count();
		let remote_count = paths.len() - local_count;

		if remote_count == 0 {
			"Local deletion".to_string()
		} else if local_count == 0 {
			// Count unique remote devices
			let mut devices = std::collections::HashSet::new();
			for path in paths {
				if let Some(device_id) = path.device_id() {
					devices.insert(device_id);
				}
			}
			format!("Remote deletion ({} devices)", devices.len())
		} else {
			format!(
				"Mixed deletion ({} local, {} remote)",
				local_count, remote_count
			)
		}
	}
}
