use super::{
	error::VolumeError,
	types::{MountType, Volume, VolumeEvent, VolumeOptions},
	watcher::{VolumeWatcher, WatcherState},
};

use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::broadcast;
use tokio::time::Instant;
use tracing::{debug, error, event, instrument};
use uuid::Uuid;

// const OPERATION_TIMEOUT: Duration = Duration::from_secs(30);

/// Manages the state of all volumes
#[derive(Debug)]
pub struct VolumeManagerState {
	/// All tracked volumes by fingerprint
	pub volumes: HashMap<Vec<u8>, Volume>,
	/// Volume manager options
	pub options: VolumeOptions,
	/// Event broadcaster
	event_tx: broadcast::Sender<VolumeEvent>,
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
			options,
			event_tx,
			last_scan: Instant::now(),
		})
	}

	/// Scans the system for volumes and updates the state
	/// This happens on startup, and during the volume manager's maintenance task
	pub async fn scan_volumes(&mut self, device_pub_id: Vec<u8>) -> Result<(), VolumeError> {
		debug!("Scanning for volumes...");
		let detected_volumes = super::os::get_volumes().await?;
		debug!("Found {} volumes during scan", detected_volumes.len());

		let mut new_state = HashMap::new(); // New state to build with detected volumes

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

	/// Gets a volume by ID (unsure if used)
	#[instrument(skip(self))]
	pub async fn get_volume(&self, volume_id: &[u8]) -> Result<Volume, VolumeError> {
		match self.volumes.get(volume_id) {
			Some(volume) => Ok(volume.clone()),
			None => {
				// Try to get from database
				// let volume = self
				// 	.library
				// 	.db
				// 	.volume()
				// 	.find_unique(volume::pub_id::equals(volume_id.to_vec()))
				// 	.exec()
				// 	.await?
				// 	.ok_or_else(|| VolumeError::InvalidId(hex::encode(volume_id)))?;
				// Ok(Volume::from(volume))
				unimplemented!()
			}
		}
	}

	/// Updates a volume's information
	#[instrument(skip(self, volume))]
	pub async fn update_volume(
		&mut self,
		volume: Volume,
		current_device_pub_id: Vec<u8>,
	) -> Result<(), VolumeError> {
		let fingerprint = volume.generate_fingerprint(current_device_pub_id);

		if let Some(old_volume) = self.volumes.get(&fingerprint) {
			if old_volume != &volume {
				// Convert immutable borrow of `self.volumes` into owned data
				let old_volume_cloned = old_volume.clone();

				// Update in memory
				self.volumes.insert(fingerprint, volume.clone());

				// Emit event
				self.emit_event(VolumeEvent::VolumeUpdated {
					old: old_volume_cloned,
					new: volume,
				})
				.await;
			}
		}

		Ok(())
	}

	/// Helper to emit events
	async fn emit_event(&self, event: VolumeEvent) {
		if let Err(e) = self.event_tx.send(event) {
			error!(?e, "Failed to emit volume event");
		}
	}

	/// Performs maintenance tasks
	pub async fn maintenance(&mut self, device_pub_id: Vec<u8>) -> Result<(), VolumeError> {
		// Rescan volumes periodically
		if self.last_scan.elapsed() > Duration::from_secs(300) {
			self.scan_volumes(device_pub_id).await?;
		}

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
