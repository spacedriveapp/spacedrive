//! Volume Manager - Central management for all volume operations

use crate::infrastructure::events::{Event, EventBus};
use crate::volume::{
	error::{VolumeError, VolumeResult},
	os_detection,
	types::{Volume, VolumeDetectionConfig, VolumeFingerprint, VolumeInfo},
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info, instrument, warn};

/// Central manager for volume detection, monitoring, and operations
pub struct VolumeManager {
	/// Currently known volumes, indexed by fingerprint
	volumes: Arc<RwLock<HashMap<VolumeFingerprint, Volume>>>,

	/// Cache mapping paths to volume fingerprints for fast lookup
	path_cache: Arc<RwLock<HashMap<PathBuf, VolumeFingerprint>>>,

	/// Configuration for volume detection
	config: VolumeDetectionConfig,

	/// Event bus for emitting volume events
	events: Arc<EventBus>,

	/// Whether the manager is currently running monitoring
	is_monitoring: Arc<RwLock<bool>>,
}

impl VolumeManager {
	/// Create a new volume manager
	pub fn new(config: VolumeDetectionConfig, events: Arc<EventBus>) -> Self {
		Self {
			volumes: Arc::new(RwLock::new(HashMap::new())),
			path_cache: Arc::new(RwLock::new(HashMap::new())),
			config,
			events,
			is_monitoring: Arc::new(RwLock::new(false)),
		}
	}

	/// Initialize the volume manager and perform initial detection
	#[instrument(skip(self))]
	pub async fn initialize(&self) -> VolumeResult<()> {
		info!("Initializing volume manager");

		// Perform initial volume detection
		self.refresh_volumes().await?;

		// Start monitoring if configured
		if self.config.refresh_interval_secs > 0 {
			self.start_monitoring().await;
		}

		info!(
			"Volume manager initialized with {} volumes",
			self.volumes.read().await.len()
		);

		Ok(())
	}

	/// Start background monitoring of volume changes
	pub async fn start_monitoring(&self) {
		if *self.is_monitoring.read().await {
			warn!("Volume monitoring already started");
			return;
		}

		*self.is_monitoring.write().await = true;

		let volumes = self.volumes.clone();
		let path_cache = self.path_cache.clone();
		let events = self.events.clone();
		let config = self.config.clone();
		let is_monitoring = self.is_monitoring.clone();

		tokio::spawn(async move {
			info!(
				"Starting volume monitoring (refresh every {}s)",
				config.refresh_interval_secs
			);

			let mut interval =
				tokio::time::interval(Duration::from_secs(config.refresh_interval_secs));

			while *is_monitoring.read().await {
				interval.tick().await;

				if let Err(e) =
					Self::refresh_volumes_internal(&volumes, &path_cache, &events, &config).await
				{
					error!("Error during volume refresh: {}", e);
				}
			}

			info!("Volume monitoring stopped");
		});
	}

	/// Stop background monitoring
	pub async fn stop_monitoring(&self) {
		*self.is_monitoring.write().await = false;
		info!("Volume monitoring stopped");
	}

	/// Refresh all volumes and detect changes
	#[instrument(skip(self))]
	pub async fn refresh_volumes(&self) -> VolumeResult<()> {
		Self::refresh_volumes_internal(&self.volumes, &self.path_cache, &self.events, &self.config)
			.await
	}

	/// Internal implementation of volume refresh
	async fn refresh_volumes_internal(
		volumes: &Arc<RwLock<HashMap<VolumeFingerprint, Volume>>>,
		path_cache: &Arc<RwLock<HashMap<PathBuf, VolumeFingerprint>>>,
		events: &Arc<EventBus>,
		config: &VolumeDetectionConfig,
	) -> VolumeResult<()> {
		debug!("Refreshing volumes");

		// Detect current volumes
		let detected_volumes = os_detection::detect_volumes(config).await?;
		let mut current_volumes = volumes.write().await;
		let mut cache = path_cache.write().await;

		// Track which volumes we've seen in this refresh
		let mut seen_fingerprints = std::collections::HashSet::new();

		// Process detected volumes
		for detected in detected_volumes {
			let fingerprint = detected.fingerprint.clone();
			seen_fingerprints.insert(fingerprint.clone());

			match current_volumes.get(&fingerprint) {
				Some(existing) => {
					// Volume exists - check for changes
					let old_info = VolumeInfo::from(existing);
					let new_info = VolumeInfo::from(&detected);

					if old_info.is_mounted != new_info.is_mounted
						|| old_info.total_bytes_available != new_info.total_bytes_available
						|| old_info.error_status != new_info.error_status
					{
						// Update the volume
						let mut updated_volume = detected.clone();
						updated_volume.update_info(new_info.clone());
						current_volumes.insert(fingerprint.clone(), updated_volume);

						// Emit update event
						events.emit(Event::VolumeUpdated {
							fingerprint: fingerprint.clone(),
							old_info: old_info.clone(),
							new_info: new_info.clone(),
						});

						// Emit mount status change if applicable
						if old_info.is_mounted != new_info.is_mounted {
							events.emit(Event::VolumeMountChanged {
								fingerprint: fingerprint.clone(),
								is_mounted: new_info.is_mounted,
							});
						}
					}
				}
				None => {
					// New volume discovered
					info!("New volume discovered: {}", detected.name);

					// Update cache for all mount points
					cache.insert(detected.mount_point.clone(), fingerprint.clone());
					for mount_point in &detected.mount_points {
						cache.insert(mount_point.clone(), fingerprint.clone());
					}

					current_volumes.insert(fingerprint.clone(), detected.clone());

					// Emit volume added event
					events.emit(Event::VolumeAdded(detected));
				}
			}
		}

		// Check for removed volumes
		let removed_fingerprints: Vec<_> = current_volumes
			.keys()
			.filter(|fp| !seen_fingerprints.contains(fp))
			.cloned()
			.collect();

		for fingerprint in removed_fingerprints {
			if let Some(removed_volume) = current_volumes.remove(&fingerprint) {
				info!("Volume removed: {}", removed_volume.name);

				// Clean up cache entries
				cache.retain(|_, fp| fp != &fingerprint);

				// Emit volume removed event
				events.emit(Event::VolumeRemoved { fingerprint });
			}
		}

		debug!("Volume refresh completed");
		Ok(())
	}

	/// Get volume information for a specific path
	#[instrument(skip(self))]
	pub async fn volume_for_path(&self, path: &Path) -> Option<Volume> {
		// Check cache first
		{
			let cache = self.path_cache.read().await;
			if let Some(fingerprint) = cache.get(path) {
				let volumes = self.volumes.read().await;
				if let Some(volume) = volumes.get(fingerprint) {
					return Some(volume.clone());
				}
			}
		}

		// Search through all volumes
		let volumes = self.volumes.read().await;
		for volume in volumes.values() {
			if volume.contains_path(&path.to_path_buf()) {
				// Cache the result
				let mut cache = self.path_cache.write().await;
				cache.insert(path.to_path_buf(), volume.fingerprint.clone());
				return Some(volume.clone());
			}
		}

		debug!("No volume found for path: {}", path.display());
		None
	}

	/// Get all currently known volumes
	pub async fn get_all_volumes(&self) -> Vec<Volume> {
		self.volumes.read().await.values().cloned().collect()
	}

	/// Get a specific volume by fingerprint
	pub async fn get_volume(&self, fingerprint: &VolumeFingerprint) -> Option<Volume> {
		self.volumes.read().await.get(fingerprint).cloned()
	}

	/// Check if two paths are on the same volume
	pub async fn same_volume(&self, path1: &Path, path2: &Path) -> bool {
		let vol1 = self.volume_for_path(path1).await;
		let vol2 = self.volume_for_path(path2).await;

		match (vol1, vol2) {
			(Some(v1), Some(v2)) => v1.fingerprint == v2.fingerprint,
			_ => false,
		}
	}

	/// Find volumes with available space
	pub async fn volumes_with_space(&self, required_bytes: u64) -> Vec<Volume> {
		self.volumes
			.read()
			.await
			.values()
			.filter(|vol| vol.total_bytes_available >= required_bytes)
			.cloned()
			.collect()
	}

	/// Get volume statistics
	pub async fn get_statistics(&self) -> VolumeStatistics {
		let volumes = self.volumes.read().await;

		let total_volumes = volumes.len();
		let mounted_volumes = volumes.values().filter(|v| v.is_mounted).count();
		let total_capacity: u64 = volumes.values().map(|v| v.total_bytes_capacity).sum();
		let total_available: u64 = volumes.values().map(|v| v.total_bytes_available).sum();

		let mut by_type = HashMap::new();
		let mut by_filesystem = HashMap::new();

		for volume in volumes.values() {
			*by_type.entry(volume.disk_type.clone()).or_insert(0) += 1;
			*by_filesystem.entry(volume.file_system.clone()).or_insert(0) += 1;
		}

		VolumeStatistics {
			total_volumes,
			mounted_volumes,
			total_capacity,
			total_available,
			by_type,
			by_filesystem,
		}
	}

	/// Run speed test on a specific volume
	#[instrument(skip(self))]
	pub async fn run_speed_test(&self, fingerprint: &VolumeFingerprint) -> VolumeResult<()> {
		let mut volumes = self.volumes.write().await;

		if let Some(volume) = volumes.get_mut(fingerprint) {
			info!("Running speed test on volume: {}", volume.name);

			match crate::volume::speed::run_speed_test(volume).await {
				Ok((read_speed, write_speed)) => {
					volume.read_speed_mbps = Some(read_speed);
					volume.write_speed_mbps = Some(write_speed);

					// Emit speed test event
					self.events.emit(Event::VolumeSpeedTested {
						fingerprint: fingerprint.clone(),
						read_speed_mbps: read_speed,
						write_speed_mbps: write_speed,
					});

					info!(
						"Speed test completed: {}MB/s read, {}MB/s write",
						read_speed, write_speed
					);

					Ok(())
				}
				Err(e) => {
					error!("Speed test failed for volume {}: {}", volume.name, e);

					// Emit error event
					self.events.emit(Event::VolumeError {
						fingerprint: fingerprint.clone(),
						error: format!("Speed test failed: {}", e),
					});

					Err(e)
				}
			}
		} else {
			Err(VolumeError::NotFound(fingerprint.to_string()))
		}
	}

	/// Clear the path cache (useful after major volume changes)
	pub async fn clear_cache(&self) {
		self.path_cache.write().await.clear();
		debug!("Volume path cache cleared");
	}

	/// Track a volume in the database
	pub async fn track_volume(
		&self,
		fingerprint: &VolumeFingerprint,
		library: &crate::library::Library,
		display_name: Option<String>,
	) -> VolumeResult<()> {
		let volumes = self.volumes.read().await;

		if let Some(runtime_volume) = volumes.get(fingerprint) {
			// Convert runtime volume to domain volume
			let device_id = crate::shared::types::get_current_device_id();
			let mut domain_volume =
				crate::domain::volume::Volume::from_runtime_volume(runtime_volume, device_id);

			// Track the volume for this library
			domain_volume.track(Some(library.id()));

			// Set custom display name if provided
			if let Some(name) = display_name {
				domain_volume.set_display_preferences(Some(name), None, None);
			}

			// TODO: Save to database via library context
			// library_ctx.db.volume().create(domain_volume).await?;

			info!(
				"Tracked volume '{}' for library '{}'",
				domain_volume.display_name(),
				library.name().await
			);

			// Emit tracking event
			self.events
				.emit(crate::infrastructure::events::Event::Custom {
					event_type: "VolumeTracked".to_string(),
					data: serde_json::json!({
						"fingerprint": fingerprint.to_string(),
						"library_id": library.id(),
						"volume_name": domain_volume.display_name(),
					}),
				});

			Ok(())
		} else {
			Err(VolumeError::NotFound(fingerprint.to_string()))
		}
	}

	/// Untrack a volume from the database
	pub async fn untrack_volume(
		&self,
		fingerprint: &VolumeFingerprint,
		library: &crate::library::Library,
	) -> VolumeResult<()> {
		// TODO: Update database to mark as untracked
		// library_ctx.db.volume().untrack(fingerprint).await?;

		info!(
			"Untracked volume '{}' from library '{}'",
			fingerprint.to_string(),
			library.name().await
		);

		// Emit untracking event
		self.events
			.emit(crate::infrastructure::events::Event::Custom {
				event_type: "VolumeUntracked".to_string(),
				data: serde_json::json!({
					"fingerprint": fingerprint.to_string(),
					"library_id": library.id(),
				}),
			});

		Ok(())
	}

	/// Get tracked volumes for a library
	pub async fn get_tracked_volumes(
		&self,
		library: &crate::library::Library,
	) -> VolumeResult<Vec<crate::domain::volume::Volume>> {
		// TODO: Query database for tracked volumes
		// library_ctx.db.volume().find_by_library(library.id()).await

		debug!(
			"Getting tracked volumes for library '{}'",
			library.name().await
		);
		Ok(Vec::new())
	}

	/// Check if a volume is tracked in any library
	pub async fn is_volume_tracked(&self, fingerprint: &VolumeFingerprint) -> VolumeResult<bool> {
		// TODO: Query database to check if volume is tracked
		// This would check across all libraries on this device
		debug!(
			"Checking if volume '{}' is tracked",
			fingerprint.to_string()
		);
		Ok(false)
	}
}

/// Statistics about detected volumes
#[derive(Debug, Clone)]
pub struct VolumeStatistics {
	pub total_volumes: usize,
	pub mounted_volumes: usize,
	pub total_capacity: u64,
	pub total_available: u64,
	pub by_type: HashMap<crate::volume::types::DiskType, usize>,
	pub by_filesystem: HashMap<crate::volume::types::FileSystem, usize>,
}

impl Drop for VolumeManager {
	fn drop(&mut self) {
		// Ensure monitoring is stopped when manager is dropped
		let is_monitoring = self.is_monitoring.clone();
		tokio::spawn(async move {
			*is_monitoring.write().await = false;
		});
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn create_test_events() -> Arc<EventBus> {
		Arc::new(EventBus::default())
	}

	#[tokio::test]
	async fn test_volume_manager_creation() {
		let config = VolumeDetectionConfig::default();
		let events = create_test_events();
		let manager = VolumeManager::new(config, events);

		let stats = manager.get_statistics().await;
		assert_eq!(stats.total_volumes, 0);
	}

	#[tokio::test]
	async fn test_volume_path_lookup() {
		let config = VolumeDetectionConfig::default();
		let events = create_test_events();
		let manager = VolumeManager::new(config, events);

		// Initially no volumes
		let volume = manager
			.volume_for_path(&PathBuf::from("/nonexistent"))
			.await;
		assert!(volume.is_none());
	}

	#[tokio::test]
	async fn test_same_volume_check() {
		let config = VolumeDetectionConfig::default();
		let events = create_test_events();
		let manager = VolumeManager::new(config, events);

		// Both paths don't exist, so should return false
		let same = manager
			.same_volume(&PathBuf::from("/path1"), &PathBuf::from("/path2"))
			.await;
		assert!(!same);
	}
}
