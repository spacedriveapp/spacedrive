use crate::{
	library::Library,
	volume::{
		speed::SpeedTest,
		types::{LibraryId, Volume, VolumeEvent, VolumeFingerprint, VolumePubId},
	},
};

use sd_core_sync::DevicePubId;
use std::{clone, collections::HashSet};
use std::{collections::HashMap, sync::Arc, time::Instant};
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, error};

use super::{VolumeError, VolumeOptions};
// Core volume registry
pub struct VolumeRegistry {
	volumes: HashMap<VolumeFingerprint, Volume>,
	device_id: DevicePubId,
}

impl VolumeRegistry {
	pub fn new(device_id: DevicePubId) -> Self {
		Self {
			volumes: HashMap::new(),
			device_id,
		}
	}

	pub fn register_volume(&mut self, mut volume: Volume) -> (Volume, VolumeFingerprint) {
		let fingerprint = VolumeFingerprint::new(&self.device_id, &volume);
		debug!(
			"Registering volume {} with fingerprint {}",
			volume.name, fingerprint
		);
		volume.fingerprint = Some(fingerprint.clone());
		self.volumes.insert(fingerprint.clone(), volume.clone());
		(volume, fingerprint)
	}

	pub fn get_volume(&self, id: &VolumeFingerprint) -> Option<&Volume> {
		self.volumes.get(id)
	}

	pub fn volumes(&self) -> impl Iterator<Item = (&VolumeFingerprint, &Volume)> {
		self.volumes.iter()
	}

	pub fn remove_volume(&mut self, id: &VolumeFingerprint) -> Option<Volume> {
		self.volumes.remove(id)
	}

	pub fn update_volume(&mut self, volume: Volume) -> VolumeFingerprint {
		let fingerprint = VolumeFingerprint::new(&self.device_id, &volume);
		self.volumes.insert(fingerprint.clone(), volume);
		fingerprint
	}

	pub fn get_volume_mut(&mut self, id: &VolumeFingerprint) -> Option<&mut Volume> {
		self.volumes.get_mut(id)
	}
}

// Library volume mapping
#[derive(Default)]
pub struct LibraryVolumeRegistry {
	// LibraryId -> (VolumeFingerprint -> VolumePubId)
	mappings: HashMap<LibraryId, HashMap<VolumeFingerprint, VolumePubId>>,
}

impl LibraryVolumeRegistry {
	pub fn new() -> Self {
		Self {
			mappings: HashMap::new(),
		}
	}

	pub fn register_library(&mut self, library_id: LibraryId) {
		self.mappings.entry(library_id).or_default();
	}

	pub fn track_volume(
		&mut self,
		library_id: LibraryId,
		fingerprint: VolumeFingerprint,
		pub_id: VolumePubId,
	) {
		if let Some(mapping) = self.mappings.get_mut(&library_id) {
			mapping.insert(fingerprint, pub_id);
		}
	}

	pub fn get_volume_id(
		&self,
		library_id: &LibraryId,
		fingerprint: &VolumeFingerprint,
	) -> Option<&VolumePubId> {
		self.mappings.get(library_id)?.get(fingerprint)
	}

	///
	pub fn untrack_volume(&mut self, library_id: &LibraryId, fingerprint: &VolumeFingerprint) {
		if let Some(mapping) = self.mappings.get_mut(library_id) {
			mapping.remove(fingerprint);
		}
	}

	// Removes all mappings for a library
	pub fn remove_library(
		&mut self,
		library_id: &LibraryId,
	) -> Vec<(VolumeFingerprint, VolumePubId)> {
		self.mappings
			.remove(library_id)
			.map(|m| m.into_iter().collect())
			.unwrap_or_default()
	}
}

// Main state manager
pub struct VolumeManagerState {
	pub registry: Arc<RwLock<VolumeRegistry>>,
	pub library_registry: Arc<RwLock<LibraryVolumeRegistry>>,
	options: VolumeOptions,
	event_tx: broadcast::Sender<VolumeEvent>,
	last_scan: Instant,
}

impl VolumeManagerState {
	pub fn new(
		device_id: DevicePubId,
		options: VolumeOptions,
		event_tx: broadcast::Sender<VolumeEvent>,
	) -> Self {
		Self {
			registry: Arc::new(RwLock::new(VolumeRegistry::new(device_id))),
			library_registry: Arc::new(RwLock::new(LibraryVolumeRegistry::new())),
			options,
			event_tx,
			last_scan: Instant::now(),
		}
	}

	pub async fn scan_volumes(&mut self) -> Result<(), VolumeError> {
		let detected_volumes = super::os::get_volumes().await?;
		let mut registry = self.registry.write().await;

		// Track existing volumes for removal detection
		let existing: HashSet<_> = registry.volumes().map(|(id, _)| id.clone()).collect();
		let mut seen = HashSet::new();

		// Process detected volumes
		for volume in detected_volumes {
			let (volume, fingerprint) = registry.register_volume(volume.clone());
			seen.insert(fingerprint.clone());

			// Emit event for new volumes
			if !existing.contains(&fingerprint) {
				let event_tx = self.event_tx.clone();
				let _ = event_tx.send(VolumeEvent::VolumeAdded(volume.clone()));

				let mut volume_clone = volume.clone();
				let event_tx = self.event_tx.clone();
				drop(registry);

				tokio::spawn(async move {
					if let Err(e) = volume_clone.speed_test(None, Some(&event_tx)).await {
						error!(?e, "Failed to perform speed test for volume");
					}
				});

				registry = self.registry.write().await;
			}
		}

		// Find and remove volumes that no longer exist
		for fingerprint in existing.difference(&seen) {
			if let Some(volume) = registry.remove_volume(fingerprint) {
				let _ = self.event_tx.send(VolumeEvent::VolumeRemoved(volume));
			}
		}

		self.last_scan = Instant::now();
		Ok(())
	}

	pub async fn register_with_library(
		&self,
		library_id: LibraryId,
		volume: &Volume,
		library: Arc<Library>,
	) -> Result<(), VolumeError> {
		let device_id = self.registry.read().await.device_id.clone();
		let fingerprint = VolumeFingerprint::new(&device_id, volume);

		// Create in database
		volume.create(&library.db, device_id.to_db()).await?;

		// Track the relationship
		self.library_registry.write().await.track_volume(
			library_id,
			fingerprint,
			VolumePubId::from(volume.pub_id.clone().unwrap()),
		);

		Ok(())
	}

	pub async fn get_volume_pub_id(
		&self,
		library_id: &LibraryId,
		fingerprint: &VolumeFingerprint,
	) -> Option<VolumePubId> {
		self.library_registry
			.read()
			.await
			.get_volume_id(library_id, fingerprint)
			.cloned()
	}

	pub async fn get_volume(&self, fingerprint: &VolumeFingerprint) -> Option<Volume> {
		self.registry.read().await.get_volume(fingerprint).cloned()
	}

	pub async fn list_volumes(&self) -> Vec<Volume> {
		self.registry
			.read()
			.await
			.volumes()
			.map(|(_, v)| v.clone())
			.collect()
	}

	pub async fn get_volumes_for_library(
		&self,
		library_id: LibraryId,
	) -> Result<Vec<Volume>, VolumeError> {
		let registry = self.registry.read().await;
		let library_registry = self.library_registry.read().await;

		let mut volumes = Vec::new();

		for (fingerprint, volume) in registry.volumes() {
			debug!("Processing volume: {:?}", volume);
			let mut volume = volume.clone();

			// Update volume with library-specific pub_id if available
			if let Some(pub_id) = library_registry.get_volume_id(&library_id, fingerprint) {
				volume.pub_id = Some(pub_id.clone().into());
			}

			volumes.push(volume);
		}

		Ok(volumes)
	}

	pub async fn volume_exists(&self, fingerprint: &VolumeFingerprint) -> bool {
		self.registry.read().await.get_volume(fingerprint).is_some()
	}

	pub async fn update_mount_status(
		&self,
		fingerprint: &VolumeFingerprint,
		is_mounted: bool,
	) -> Result<(), VolumeError> {
		let volume = self
			.get_volume(fingerprint)
			.await
			.ok_or_else(|| VolumeError::NotFound(fingerprint.clone()))?;

		let _ = self.event_tx.send(VolumeEvent::VolumeMountChanged {
			fingerprint: fingerprint.clone(),
			is_mounted,
		});
		Ok(())
	}
	// pub async fn get_statistics(&self) -> VolumeStats {
	// 	VolumeStats {
	// 		total_volumes: self.registry.read().await.volumes.len(),
	// 		tracked_libraries: self.library_registry.read().await.mappings.len(),
	// 		last_scan_age: self.last_scan.elapsed(),
	// 	}
	// }
}
