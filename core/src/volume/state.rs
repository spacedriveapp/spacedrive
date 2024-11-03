use super::{
	error::VolumeError,
	types::{Volume, VolumeEvent, VolumeOptions},
};

use std::{collections::HashMap, time::Duration};
use tokio::sync::broadcast;
use tokio::time::Instant;
use tracing::{debug, error, instrument};

// const OPERATION_TIMEOUT: Duration = Duration::from_secs(30);

/// Manages the state of all volumes
#[derive(Debug)]
pub struct VolumeManagerState {
	/// All tracked volumes by fingerprint
	pub volumes: HashMap<Vec<u8>, Volume>,
	/// Mapping of library volumes to system volumes
	/// LibraryPubId -> VolumePubId -> Fingerprint
	pub library_volume_mapping: HashMap<Vec<u8>, HashMap<Vec<u8>, Vec<u8>>>,
	/// Volume manager options
	pub options: VolumeOptions,
	/// Event broadcaster
	_event_tx: broadcast::Sender<VolumeEvent>,
	/// Last scan time
	pub last_scan: Instant,
}

impl VolumeManagerState {
	/// Creates a new volume manager
	// Take event_tx as parameter instead of creating new channel
	pub async fn new(
		options: VolumeOptions,
		event_tx: broadcast::Sender<VolumeEvent>,
	) -> Result<Self, VolumeError> {
		Ok(Self {
			volumes: HashMap::new(),
			library_volume_mapping: HashMap::new(),
			options,
			_event_tx: event_tx,
			last_scan: Instant::now(),
		})
	}

	/// Scans the system for volumes and updates the state
	/// This happens on startup, and during the volume manager's maintenance task
	pub async fn scan_volumes(&mut self, device_pub_id: Vec<u8>) -> Result<(), VolumeError> {
		let detected_volumes = super::os::get_volumes().await?;
		// New state to build with detected volumes
		let mut new_state = HashMap::new();
		// Process each detected volume
		for volume in detected_volumes {
			// Generate a unique fingerprint to identify the volume
			let fingerprint = volume.generate_fingerprint(device_pub_id.clone());
			// Insert volume into new state (whether new or updated)
			new_state.insert(fingerprint, volume);
		}

		// Update the volume manager's state with the new volume list
		self.volumes = new_state;
		self.last_scan = Instant::now(); // Update the last scan time
		Ok(())
	}
}

// #[cfg(test)]
// mod tests {
// 	use super::*;
// 	use tempfile::tempdir;

// 	#[tokio::test]
// 	async fn test_volume_management() {
// 		let node = Arc::new(Node::default());
// 		let options = VolumeOptions::default();
// 		let mut state = VolumeManagerState::new(node, options).await.unwrap();

// 		// Test volume scanning
// 		state.scan_volumes().await.unwrap();
// 		assert!(
// 			!state.volumes.is_empty(),
// 			"Should detect at least one volume"
// 		);

// 		// Test volume watching
// 		if let Some((volume_id, _)) = state.volumes.iter().next() {
// 			state.watch_volume(volume_id.clone()).await.unwrap();
// 			assert!(state.watchers.contains_key(volume_id));

// 			// Test watcher pausing
// 			state.pause_watcher(volume_id).await.unwrap();
// 			assert!(state.watchers.get(volume_id).unwrap().paused);

// 			// Test watcher resuming
// 			state.resume_watcher(volume_id).await.unwrap();
// 			assert!(!state.watchers.get(volume_id).unwrap().paused);

// 			// Test unwatching
// 			state.unwatch_volume(volume_id).await.unwrap();
// 			assert!(!state.watchers.contains_key(volume_id));
// 		}
// 	}
// }
