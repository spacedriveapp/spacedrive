//! Volume Manager - Central management for all volume operations

use crate::infrastructure::database::entities;
use crate::infrastructure::events::{Event, EventBus};
use crate::library::LibraryManager;
use crate::volume::{
	error::{VolumeError, VolumeResult},
	os_detection,
	types::{TrackedVolume, Volume, VolumeDetectionConfig, VolumeFingerprint, VolumeInfo},
};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, Set};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info, instrument, warn};
use uuid::Uuid;

/// Central manager for volume detection, monitoring, and operations
pub struct VolumeManager {
	/// Device ID for this manager
	device_id: uuid::Uuid,

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

	/// Weak reference to library manager for database operations
	library_manager: RwLock<Option<Weak<LibraryManager>>>,
}

impl VolumeManager {
	/// Create a new VolumeManager instance
	pub fn new(
		device_id: uuid::Uuid,
		config: VolumeDetectionConfig,
		events: Arc<EventBus>,
	) -> Self {
		Self {
			device_id,
			volumes: Arc::new(RwLock::new(HashMap::new())),
			path_cache: Arc::new(RwLock::new(HashMap::new())),
			config,
			events,
			is_monitoring: Arc::new(RwLock::new(false)),
			library_manager: RwLock::new(None),
		}
	}

	/// Set the library manager reference
	pub async fn set_library_manager(&self, library_manager: Weak<LibraryManager>) {
		*self.library_manager.write().await = Some(library_manager);
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
		let device_id = self.device_id;

		tokio::spawn(async move {
			info!(
				"Starting volume monitoring (refresh every {}s)",
				config.refresh_interval_secs
			);

			let mut interval =
				tokio::time::interval(Duration::from_secs(config.refresh_interval_secs));

			while *is_monitoring.read().await {
				interval.tick().await;

				if let Err(e) = Self::refresh_volumes_internal(
					device_id,
					&volumes,
					&path_cache,
					&events,
					&config,
				)
				.await
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
		Self::refresh_volumes_internal(
			self.device_id,
			&self.volumes,
			&self.path_cache,
			&self.events,
			&self.config,
		)
		.await
	}

	/// Internal implementation of volume refresh
	async fn refresh_volumes_internal(
		device_id: uuid::Uuid,
		volumes: &Arc<RwLock<HashMap<VolumeFingerprint, Volume>>>,
		path_cache: &Arc<RwLock<HashMap<PathBuf, VolumeFingerprint>>>,
		events: &Arc<EventBus>,
		config: &VolumeDetectionConfig,
	) -> VolumeResult<()> {
		debug!("Refreshing volumes for device {}", device_id);

		// Detect current volumes
		let detected_volumes = os_detection::detect_volumes(device_id, config).await?;
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
		// Canonicalize the path to handle relative paths properly
		let canonical_path = match path.canonicalize() {
			Ok(p) => p,
			Err(e) => {
				debug!("Failed to canonicalize path {}: {}", path.display(), e);
				// If canonicalization fails, try with the original path
				path.to_path_buf()
			}
		};

		// Check cache first (use canonical path for cache key)
		{
			let cache = self.path_cache.read().await;
			if let Some(fingerprint) = cache.get(&canonical_path) {
				let volumes = self.volumes.read().await;
				if let Some(volume) = volumes.get(fingerprint) {
					return Some(volume.clone());
				}
			}
		}

		// Search through all volumes using canonical path
		let volumes = self.volumes.read().await;
		for volume in volumes.values() {
			if volume.contains_path(&canonical_path) {
				// Cache the result using canonical path
				let mut cache = self.path_cache.write().await;
				cache.insert(canonical_path.clone(), volume.fingerprint.clone());
				return Some(volume.clone());
			}
		}

		debug!("No volume found for path: {}", canonical_path.display());
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
		library: &crate::library::Library,
		fingerprint: &VolumeFingerprint,
		display_name: Option<String>,
	) -> VolumeResult<entities::volume::Model> {
		let db = library.db().conn();

		// Check if already tracked
		if let Some(existing) = entities::volume::Entity::find()
			.filter(entities::volume::Column::Fingerprint.eq(fingerprint.0.clone()))
			.one(db)
			.await
			.map_err(|e| VolumeError::Database(e.to_string()))?
		{
			return Err(VolumeError::AlreadyTracked(fingerprint.to_string()));
		}

		// Get current volume info
		let volume = self
			.get_volume(fingerprint)
			.await
			.ok_or_else(|| VolumeError::NotFound(fingerprint.to_string()))?;

		// Determine removability and network status
		let is_removable = matches!(volume.mount_type, crate::volume::types::MountType::External);
		let is_network_drive =
			matches!(volume.mount_type, crate::volume::types::MountType::Network);

		// Create tracking record
		let active_model = entities::volume::ActiveModel {
			uuid: Set(Uuid::new_v4()),
			device_id: Set(volume.device_id), // Use Uuid directly
			fingerprint: Set(fingerprint.0.clone()),
			display_name: Set(display_name.clone()),
			tracked_at: Set(chrono::Utc::now()),
			last_seen_at: Set(chrono::Utc::now()),
			is_online: Set(volume.is_mounted),
			total_capacity: Set(Some(volume.total_bytes_capacity as i64)),
			available_capacity: Set(Some(volume.total_bytes_available as i64)),
			read_speed_mbps: Set(volume.read_speed_mbps.map(|s| s as i32)),
			write_speed_mbps: Set(volume.write_speed_mbps.map(|s| s as i32)),
			last_speed_test_at: Set(None),
			file_system: Set(Some(volume.file_system.to_string())),
			mount_point: Set(Some(volume.mount_point.to_string_lossy().to_string())),
			is_removable: Set(Some(is_removable)),
			is_network_drive: Set(Some(is_network_drive)),
			device_model: Set(volume.hardware_id.clone()),
			..Default::default()
		};

		let model = active_model
			.insert(db)
			.await
			.map_err(|e| VolumeError::Database(e.to_string()))?;

		info!(
			"Tracked volume '{}' for library '{}'",
			display_name.as_ref().unwrap_or(&volume.name),
			library.name().await
		);

		// Emit tracking event
		self.events.emit(Event::Custom {
			event_type: "VolumeTracked".to_string(),
			data: serde_json::json!({
				"library_id": library.id(),
				"volume_fingerprint": fingerprint.to_string(),
				"display_name": display_name,
			}),
		});

		Ok(model)
	}

	/// Untrack a volume from the database
	pub async fn untrack_volume(
		&self,
		library: &crate::library::Library,
		fingerprint: &VolumeFingerprint,
	) -> VolumeResult<()> {
		let db = library.db().conn();

		let result = entities::volume::Entity::delete_many()
			.filter(entities::volume::Column::Fingerprint.eq(fingerprint.0.clone()))
			.exec(db)
			.await
			.map_err(|e| VolumeError::Database(e.to_string()))?;

		if result.rows_affected == 0 {
			return Err(VolumeError::NotTracked(fingerprint.to_string()));
		}

		info!(
			"Untracked volume '{}' from library '{}'",
			fingerprint.to_string(),
			library.name().await
		);

		// Emit untracking event
		self.events.emit(Event::Custom {
			event_type: "VolumeUntracked".to_string(),
			data: serde_json::json!({
				"library_id": library.id(),
				"volume_fingerprint": fingerprint.to_string(),
			}),
		});

		Ok(())
	}

	/// Get tracked volumes for a library
	pub async fn get_tracked_volumes(
		&self,
		library: &crate::library::Library,
	) -> VolumeResult<Vec<TrackedVolume>> {
		let db = library.db().conn();

		let volumes = entities::volume::Entity::find()
			.all(db)
			.await
			.map_err(|e| VolumeError::Database(e.to_string()))?;

		let tracked_volumes: Vec<TrackedVolume> = volumes
			.into_iter()
			.map(|model| model.to_tracked_volume())
			.collect();

		debug!(
			"Found {} tracked volumes for library '{}'",
			tracked_volumes.len(),
			library.name().await
		);

		Ok(tracked_volumes)
	}

	/// Check if a volume is tracked in a specific library
	pub async fn is_volume_tracked(
		&self,
		library: &crate::library::Library,
		fingerprint: &VolumeFingerprint,
	) -> VolumeResult<bool> {
		let db = library.db().conn();

		// Get the volume to find its device_id
		let volume = self
			.get_volume(fingerprint)
			.await
			.ok_or_else(|| VolumeError::NotFound(fingerprint.to_string()))?;

		let count = entities::volume::Entity::find()
			.filter(entities::volume::Column::DeviceId.eq(volume.device_id))
			.filter(entities::volume::Column::Fingerprint.eq(fingerprint.0.clone()))
			.count(db)
			.await
			.map_err(|e| VolumeError::Database(e.to_string()))?;

		Ok(count > 0)
	}

	/// Update tracked volume state during refresh
	pub async fn update_tracked_volume_state(
		&self,
		library: &crate::library::Library,
		fingerprint: &VolumeFingerprint,
		volume: &Volume,
	) -> VolumeResult<()> {
		let db = library.db().conn();

		let mut active_model: entities::volume::ActiveModel = entities::volume::Entity::find()
			.filter(entities::volume::Column::DeviceId.eq(volume.device_id))
			.filter(entities::volume::Column::Fingerprint.eq(fingerprint.0.clone()))
			.one(db)
			.await
			.map_err(|e| VolumeError::Database(e.to_string()))?
			.ok_or_else(|| VolumeError::NotTracked(fingerprint.to_string()))?
			.into();

		active_model.last_seen_at = Set(chrono::Utc::now());
		active_model.is_online = Set(volume.is_mounted);
		active_model.total_capacity = Set(Some(volume.total_bytes_capacity as i64));
		active_model.available_capacity = Set(Some(volume.total_bytes_available as i64));

		active_model
			.update(db)
			.await
			.map_err(|e| VolumeError::Database(e.to_string()))?;

		Ok(())
	}

	/// Update display names for tracked volumes that have empty display names
	pub async fn update_empty_display_names(
		&self,
		library: &crate::library::Library,
	) -> VolumeResult<usize> {
		let db = library.db().conn();

		// Find tracked volumes with empty display names
		let volumes_to_update = entities::volume::Entity::find()
			.filter(
				entities::volume::Column::DisplayName
					.is_null()
					.or(entities::volume::Column::DisplayName.eq("")),
			)
			.all(db)
			.await
			.map_err(|e| VolumeError::Database(e.to_string()))?;

		let mut updated_count = 0;

		for tracked_volume in volumes_to_update {
			let fingerprint = VolumeFingerprint(tracked_volume.fingerprint.clone());

			// Get the current volume info to get the name
			if let Some(volume) = self.get_volume(&fingerprint).await {
				let volume_name = volume.name.clone();
				let mut active_model: entities::volume::ActiveModel = tracked_volume.into();
				active_model.display_name = Set(Some(volume.name));

				match active_model.update(db).await {
					Ok(_) => {
						updated_count += 1;
						info!("Updated display name for volume: {}", volume_name);
					}
					Err(e) => {
						warn!(
							"Failed to update display name for volume {}: {}",
							fingerprint.0, e
						);
					}
				}
			}
		}

		info!(
			"Updated display names for {} volumes in library '{}'",
			updated_count,
			library.name().await
		);
		Ok(updated_count)
	}

	/// Get all system volumes (boot/OS volumes)
	pub async fn get_system_volumes(&self) -> Vec<Volume> {
		self.volumes
			.read()
			.await
			.values()
			.filter(|v| matches!(v.mount_type, crate::volume::types::MountType::System))
			.cloned()
			.collect()
	}

	/// Automatically track system volumes for a library
	pub async fn auto_track_system_volumes(
		&self,
		library: &crate::library::Library,
	) -> VolumeResult<Vec<entities::volume::Model>> {
		let system_volumes = self.get_system_volumes().await;
		let mut tracked_volumes = Vec::new();

		info!(
			"Auto-tracking {} system volumes for library '{}'",
			system_volumes.len(),
			library.name().await
		);

		for volume in system_volumes {
			// Skip if already tracked
			if self.is_volume_tracked(library, &volume.fingerprint).await? {
				debug!("System volume '{}' already tracked in library", volume.name);
				continue;
			}

			// Track the system volume
			match self
				.track_volume(library, &volume.fingerprint, Some(volume.name.clone()))
				.await
			{
				Ok(tracked) => {
					info!(
						"Auto-tracked system volume '{}' in library '{}'",
						volume.name,
						library.name().await
					);
					tracked_volumes.push(tracked);
				}
				Err(e) => {
					warn!(
						"Failed to auto-track system volume '{}': {}",
						volume.name, e
					);
				}
			}
		}

		Ok(tracked_volumes)
	}

	/// Save speed test results to all libraries where this volume is tracked
	pub async fn save_speed_test_results(
		&self,
		fingerprint: &VolumeFingerprint,
		read_speed_mbps: u64,
		write_speed_mbps: u64,
		libraries: &[Arc<crate::library::Library>],
	) -> VolumeResult<()> {
		for library in libraries {
			// Check if this volume is tracked in this library
			if self.is_volume_tracked(library, fingerprint).await? {
				let db = library.db().conn();

				// Get the volume to find its device_id
				let volume = self
					.get_volume(fingerprint)
					.await
					.ok_or_else(|| VolumeError::NotFound(fingerprint.to_string()))?;

				// Update the tracked volume record with speed test results
				let now = chrono::Utc::now();

				let update_result = entities::volume::Entity::update_many()
					.filter(entities::volume::Column::DeviceId.eq(volume.device_id))
					.filter(entities::volume::Column::Fingerprint.eq(fingerprint.0.clone()))
					.set(entities::volume::ActiveModel {
						read_speed_mbps: Set(Some(read_speed_mbps as i32)),
						write_speed_mbps: Set(Some(write_speed_mbps as i32)),
						last_speed_test_at: Set(Some(now)),
						..Default::default()
					})
					.exec(db)
					.await
					.map_err(|e| VolumeError::Database(e.to_string()))?;

				if update_result.rows_affected > 0 {
					info!(
						"Saved speed test results for volume {} in library {}: {}MB/s read, {}MB/s write",
						fingerprint.0,
						library.name().await,
						read_speed_mbps,
						write_speed_mbps
					);
				}
			}
		}

		Ok(())
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
		let manager = VolumeManager::new(Uuid::new_v4(), config, events);

		let stats = manager.get_statistics().await;
		assert_eq!(stats.total_volumes, 0);
	}

	#[tokio::test]
	async fn test_volume_path_lookup() {
		let config = VolumeDetectionConfig::default();
		let events = create_test_events();
		let manager = VolumeManager::new(Uuid::new_v4(), config, events);

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
		let manager = VolumeManager::new(Uuid::new_v4(), config, events);

		// Both paths don't exist, so should return false
		let same = manager
			.same_volume(&PathBuf::from("/path1"), &PathBuf::from("/path2"))
			.await;
		assert!(!same);
	}
}
