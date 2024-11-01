use super::{
	error::VolumeError,
	types::{MountType, Volume, VolumeEvent, VolumeOptions},
	watcher::{VolumeWatcher, WatcherState},
};

use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::broadcast;
use tokio::time::Instant;
use tracing::{debug, error, instrument};
use uuid::Uuid;

// const OPERATION_TIMEOUT: Duration = Duration::from_secs(30);

/// Manages the state of all volumes
#[derive(Debug)]
pub struct VolumeManagerState {
	/// All tracked volumes
	pub volumes: HashMap<Vec<u8>, Volume>,
	/// Active watchers
	pub watchers: HashMap<Vec<u8>, WatcherState>,
	/// Volume manager options
	pub options: VolumeOptions,
	/// Event broadcaster
	event_tx: broadcast::Sender<VolumeEvent>,
	/// Last scan time
	pub last_scan: Instant,
}

impl VolumeManagerState {
	/// Creates a new volume manager
	pub async fn new(options: VolumeOptions) -> Result<Self, VolumeError> {
		let (event_tx, _) = broadcast::channel(128);

		Ok(Self {
			volumes: HashMap::new(),
			watchers: HashMap::new(),
			options,
			event_tx,
			last_scan: Instant::now(),
		})
	}

	/// Scans the system for volumes and updates the state
	#[instrument(skip(self))]
	pub async fn scan_volumes(&mut self) -> Result<(), VolumeError> {
		debug!("Scanning for volumes...");
		let detected_volumes = super::os::get_volumes().await?;
		let mut new_volumes = Vec::new();
		let mut removed_volumes = self.volumes.clone();

		debug!("Found {} volumes during scan", detected_volumes.len());

		// Clear existing volumes and reinsert with new IDs
		self.volumes.clear();

		for volume in detected_volumes {
			// Skip virtual volumes if configured
			if !self.options.include_virtual && matches!(volume.mount_type, MountType::Virtual) {
				continue;
			}

			let volume_id = match &volume.pub_id {
				Some(id) => id.clone(),
				None => {
					// New volume, generate ID
					let id = Uuid::now_v7().as_bytes().to_vec();
					let mut volume = volume.clone();
					volume.pub_id = Some(id.clone());
					new_volumes.push(volume);
					id
				}
			};

			// Remove from potentially removed volumes
			removed_volumes.remove(&volume_id);

			// Update existing volume or add new one
			match self.volumes.get_mut(&volume_id) {
				Some(existing) => {
					if existing != &volume {
						let old = existing.clone();
						*existing = volume;
						let event = VolumeEvent::VolumeUpdated {
							old,
							new: existing.clone(),
						};
						self.emit_event(event).await;
					}
				}
				None => {
					self.volumes.insert(volume_id, volume.clone());
					self.emit_event(VolumeEvent::VolumeAdded(volume)).await;
				}
			}
		}

		// Handle removed volumes
		for (id, volume) in removed_volumes {
			self.volumes.remove(&id);
			self.watchers.remove(&id);
			self.emit_event(VolumeEvent::VolumeRemoved(volume)).await;
		}

		self.last_scan = Instant::now();
		Ok(())
	}

	/// Starts watching a volume
	#[instrument(skip(self))]
	pub async fn watch_volume(&mut self, volume_id: Vec<u8>) -> Result<(), VolumeError> {
		if self.watchers.contains_key(&volume_id) {
			debug!("Already watching volume {:?}", hex::encode(&volume_id));
			return Ok(());
		}

		let watcher = Arc::new(VolumeWatcher::new(self.event_tx.clone()));
		watcher.start().await?;

		self.watchers.insert(
			volume_id.clone(),
			WatcherState {
				watcher,
				last_event: Instant::now(),
				paused: false,
			},
		);

		debug!("Started watching volume {}", hex::encode(&volume_id));
		Ok(())
	}

	/// Stops watching a volume
	#[instrument(skip(self))]
	pub async fn unwatch_volume(&mut self, volume_id: &[u8]) -> Result<(), VolumeError> {
		if let Some(state) = self.watchers.remove(volume_id) {
			state.watcher.stop().await;
			debug!("Stopped watching volume {}", hex::encode(volume_id));
		}
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
	pub async fn update_volume(&mut self, volume: Volume) -> Result<(), VolumeError> {
		let volume_id = volume
			.pub_id
			.as_ref()
			.ok_or(VolumeError::NotInDatabase)?
			.clone();

		if let Some(old_volume) = self.volumes.get(&volume_id) {
			if old_volume != &volume {
				// Convert immutable borrow of `self.volumes` into owned data
				let old_volume_cloned = old_volume.clone();

				// Update in memory
				self.volumes.insert(volume_id, volume.clone());

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

	/// Temporarily pauses a volume watcher
	#[instrument(skip(self))]
	pub async fn pause_watcher(&mut self, volume_id: &[u8]) -> Result<(), VolumeError> {
		if let Some(state) = self.watchers.get_mut(volume_id) {
			if !state.paused {
				state.paused = true;
				debug!("Paused watcher for volume {}", hex::encode(volume_id));
			}
		}
		Ok(())
	}

	/// Resumes a paused volume watcher
	#[instrument(skip(self))]
	pub async fn resume_watcher(&mut self, volume_id: &[u8]) -> Result<(), VolumeError> {
		if let Some(state) = self.watchers.get_mut(volume_id) {
			if state.paused {
				state.paused = false;
				debug!("Resumed watcher for volume {}", hex::encode(volume_id));
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
	pub async fn maintenance(&mut self) -> Result<(), VolumeError> {
		// Rescan volumes periodically
		if self.last_scan.elapsed() > Duration::from_secs(300) {
			self.scan_volumes().await?;
		}

		// Clean up stale watchers
		let stale_watchers: Vec<_> = self
			.watchers
			.iter()
			.filter(|(_, state)| state.last_event.elapsed() > Duration::from_secs(3600))
			.map(|(id, _)| id.clone())
			.collect();

		for volume_id in stale_watchers {
			self.unwatch_volume(&volume_id).await?;
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
