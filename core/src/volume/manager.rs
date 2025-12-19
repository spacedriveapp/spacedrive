//! Volume Manager - Central management for all volume operations

use crate::infra::db::entities;
use crate::infra::event::{Event, EventBus};
use crate::library::LibraryManager;
use crate::volume::{
	detection,
	error::{VolumeError, VolumeResult},
	types::{
		SpacedriveVolumeId, TrackedVolume, Volume, VolumeDetectionConfig, VolumeFingerprint,
		VolumeInfo,
	},
	VolumeExt,
};
use crate::Core;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, Set};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};
use std::time::Duration as StdDuration;
use tokio::{fs, sync::RwLock, time::Duration};
use tracing::{debug, error, info, instrument, warn};
use uuid::Uuid;

/// Filename for Spacedrive volume identifier files
const SPACEDRIVE_VOLUME_ID_FILE: &str = ".spacedrive-volume-id";

/// Get platform-specific directories to watch for volume mount changes
fn get_volume_watch_paths() -> Vec<PathBuf> {
	let mut paths = Vec::new();

	#[cfg(target_os = "macos")]
	{
		paths.push(PathBuf::from("/Volumes"));
		// Note: System volumes like / and /System/Volumes/Data are typically stable
	}

	#[cfg(target_os = "linux")]
	{
		paths.push(PathBuf::from("/media"));
		paths.push(PathBuf::from("/mnt"));
		// Note: Could also watch /proc/mounts but that's more complex
	}

	#[cfg(target_os = "windows")]
	{
		// Windows drive letters are harder to watch - we'll rely on polling for now
		// Could potentially use WMI events in the future
	}

	// Filter to only existing directories
	paths.into_iter().filter(|p: &PathBuf| p.exists()).collect()
}

/// Central manager for volume detection, monitoring, and operations
pub struct VolumeManager {
	/// Device ID for this manager
	pub(crate) device_id: uuid::Uuid,

	/// Currently known volumes, indexed by fingerprint
	volumes: Arc<RwLock<HashMap<VolumeFingerprint, Volume>>>,

	/// Cache mapping paths to volume fingerprints for fast lookup
	path_cache: Arc<RwLock<HashMap<PathBuf, VolumeFingerprint>>>,

	/// Cache mapping mount points to fingerprints for O(1) cloud volume lookup
	/// Format: "s3://bucket" -> fingerprint
	mount_point_cache: Arc<RwLock<HashMap<String, VolumeFingerprint>>>,

	/// Configuration for volume detection
	config: VolumeDetectionConfig,

	/// Event bus for emitting volume events
	events: Arc<EventBus>,

	/// Whether the manager is currently running monitoring
	is_monitoring: Arc<RwLock<bool>>,

	/// File system watcher for real-time volume change detection
	volume_watcher: Arc<RwLock<Option<RecommendedWatcher>>>,

	/// Weak reference to library manager for database operations
	library_manager: Arc<RwLock<Option<Weak<LibraryManager>>>>,
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
			mount_point_cache: Arc::new(RwLock::new(HashMap::new())),
			config,
			events,
			is_monitoring: Arc::new(RwLock::new(false)),
			volume_watcher: Arc::new(RwLock::new(None)),
			library_manager: Arc::new(RwLock::new(None)),
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

		// Perform initial volume detection (for local volumes)
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

	/// Load cloud volumes from the database and restore them to the in-memory HashMap
	/// This should be called after libraries are loaded
	pub async fn load_cloud_volumes_from_db(
		&self,
		libraries: &[std::sync::Arc<crate::library::Library>],
		key_manager: std::sync::Arc<crate::crypto::key_manager::KeyManager>,
	) -> VolumeResult<()> {
		use crate::crypto::cloud_credentials::CloudCredentialManager;

		let mut loaded_count = 0;

		for library in libraries {
			let db = library.db().conn();

			// Query all network volumes (cloud volumes) for this library
			let cloud_volumes = entities::volume::Entity::find()
				.filter(entities::volume::Column::IsNetworkDrive.eq(true))
				.all(db)
				.await
				.map_err(|e| VolumeError::Database(e.to_string()))?;

			info!(
				"Found {} cloud volumes in database for library {}",
				cloud_volumes.len(),
				library.id()
			);

			for db_volume in cloud_volumes {
				let fingerprint = VolumeFingerprint(db_volume.fingerprint.clone());

				// Skip if already loaded
				if self.get_volume(&fingerprint).await.is_some() {
					continue;
				}

				// Try to load credentials and recreate the backend
				let credential_manager = CloudCredentialManager::new(
					key_manager.clone(),
					library.db().clone(),
					library.id(),
				);

				match credential_manager
					.get_credential(library.id(), &db_volume.fingerprint)
					.await
				{
					Ok(credential) => {
						// Get mount point from database (for display and cache purposes)
						let mount_point_str = match &db_volume.mount_point {
							Some(mp) => mp,
							None => {
								warn!("No mount point for cloud volume {}", fingerprint.0);
								continue;
							}
						};

						// Get cloud identifier from database (actual bucket/drive/container name)
						let cloud_identifier = match &db_volume.cloud_identifier {
							Some(id) => id,
							None => {
								warn!("No cloud identifier for cloud volume {}", fingerprint.0);
								continue;
							}
						};

						// Parse cloud_config JSON if available
						let cloud_config: Option<serde_json::Value> = db_volume
							.cloud_config
							.as_ref()
							.and_then(|s| serde_json::from_str(s).ok());

						let backend_result = match credential.service {
							crate::volume::CloudServiceType::S3 => {
								if let crate::crypto::cloud_credentials::CredentialData::AccessKey {
									access_key_id,
									secret_access_key,
									..
								} = &credential.data
								{
									// Extract region from cloud_config, or default to us-east-1
									let region = cloud_config
										.as_ref()
										.and_then(|c| c.get("region"))
										.and_then(|r| r.as_str())
										.unwrap_or("us-east-1");

									let endpoint = cloud_config
										.as_ref()
										.and_then(|c| c.get("endpoint"))
										.and_then(|e| e.as_str())
										.map(String::from);

									crate::volume::CloudBackend::new_s3(
										cloud_identifier,
										region,
										access_key_id,
										secret_access_key,
										endpoint,
									).await
								} else {
									warn!("Invalid credential type for S3 volume {}", fingerprint.0);
									continue;
								}
							}
							crate::volume::CloudServiceType::GoogleDrive => {
								if let crate::crypto::cloud_credentials::CredentialData::OAuth {
									access_token,
									refresh_token,
								} = &credential.data
								{
									crate::volume::CloudBackend::new_google_drive(
										access_token,
										refresh_token,
										"", // client_id not stored yet
										"", // client_secret not stored yet
										Some(cloud_identifier.clone()),
									).await
								} else {
									warn!("Invalid credential type for Google Drive volume {}", fingerprint.0);
									continue;
								}
							}
							crate::volume::CloudServiceType::OneDrive => {
								if let crate::crypto::cloud_credentials::CredentialData::OAuth {
									access_token,
									refresh_token,
								} = &credential.data
								{
									crate::volume::CloudBackend::new_onedrive(
										access_token,
										refresh_token,
										"",
										"",
										Some(cloud_identifier.clone()),
									).await
								} else {
									warn!("Invalid credential type for OneDrive volume {}", fingerprint.0);
									continue;
								}
							}
							crate::volume::CloudServiceType::Dropbox => {
								if let crate::crypto::cloud_credentials::CredentialData::OAuth {
									access_token,
									refresh_token,
								} = &credential.data
								{
									crate::volume::CloudBackend::new_dropbox(
										access_token,
										refresh_token,
										"",
										"",
										Some(cloud_identifier.clone()),
									).await
								} else {
									warn!("Invalid credential type for Dropbox volume {}", fingerprint.0);
									continue;
								}
							}
							crate::volume::CloudServiceType::AzureBlob => {
								if let crate::crypto::cloud_credentials::CredentialData::AccessKey {
									access_key_id,
									secret_access_key,
									..
								} = &credential.data
								{
									crate::volume::CloudBackend::new_azure_blob(
										cloud_identifier,
										access_key_id,
										secret_access_key,
										None,
									).await
								} else {
									warn!("Invalid credential type for Azure Blob volume {}", fingerprint.0);
									continue;
								}
							}
							crate::volume::CloudServiceType::GoogleCloudStorage => {
								if let crate::crypto::cloud_credentials::CredentialData::ApiKey(service_account_json) = &credential.data {
									crate::volume::CloudBackend::new_google_cloud_storage(
										cloud_identifier,
										service_account_json,
										None,
										None,
									).await
								} else {
									warn!("Invalid credential type for GCS volume {}", fingerprint.0);
									continue;
								}
							}
							_ => {
								warn!("Unsupported cloud service type {:?} for volume {}", credential.service, fingerprint.0);
								continue;
							}
						};

						match backend_result {
							Ok(backend) => {
								let now = chrono::Utc::now();

								let volume = Volume {
									id: db_volume.uuid,
									fingerprint: fingerprint.clone(),
									device_id: db_volume.device_id,
									name: db_volume
										.display_name
										.clone()
										.unwrap_or_else(|| "Cloud Volume".to_string()),
									library_id: None,
									is_tracked: true,
									mount_point: std::path::PathBuf::from(mount_point_str),
									mount_points: vec![std::path::PathBuf::from(mount_point_str)],
									volume_type: crate::volume::types::VolumeType::Network,
									mount_type: crate::volume::types::MountType::Network,
									disk_type: crate::volume::types::DiskType::Unknown,
									file_system: crate::volume::types::FileSystem::Other(format!(
										"{:?}",
										credential.service
									)),
									total_capacity: db_volume.total_capacity.unwrap_or(0) as u64,
									available_space: db_volume.available_capacity.unwrap_or(0)
										as u64,
									is_read_only: false,
									is_mounted: true,
									hardware_id: None,
									backend: Some(Arc::new(backend)),
									cloud_identifier: db_volume.cloud_identifier.clone(),
									cloud_config,
									apfs_container: None,
									container_volume_id: None,
									path_mappings: Vec::new(),
									is_user_visible: db_volume.is_user_visible.unwrap_or(true),
									auto_track_eligible: db_volume
										.auto_track_eligible
										.unwrap_or(false),
									read_speed_mbps: db_volume.read_speed_mbps.map(|s| s as u64),
									write_speed_mbps: db_volume.write_speed_mbps.map(|s| s as u64),
									created_at: db_volume.tracked_at,
									updated_at: now,
									last_seen_at: db_volume.last_seen_at,
									total_files: None,
									total_directories: None,
									last_stats_update: None,
									display_name: db_volume.display_name.clone(),
									is_favorite: false,
									color: None,
									icon: None,
									error_message: None,
								};

								let mut volumes = self.volumes.write().await;

								volumes.insert(fingerprint.clone(), volume.clone());

								// Update mount point cache for fast cloud volume lookup using cloud_identifier
								if let Some(ref cloud_id) = volume.cloud_identifier {
									let cache_key =
										format!("{}://{}", credential.service.scheme(), cloud_id);
									let mut mount_point_cache =
										self.mount_point_cache.write().await;
									mount_point_cache.insert(cache_key, fingerprint.clone());
								}

								loaded_count += 1;
								info!(
									"Loaded cloud volume {} ({:?}) from database",
									db_volume
										.display_name
										.as_ref()
										.unwrap_or(&"Unknown".to_string()),
									credential.service
								);
							}
							Err(e) => {
								warn!(
									"Failed to recreate cloud backend for volume {}: {}",
									fingerprint.0, e
								);
							}
						}
					}
					Err(e) => {
						warn!("Failed to load credentials for cloud volume {} ({}): {}. The volume will not be available until credentials are re-entered by removing and re-adding the volume.",
							db_volume.display_name.as_ref().unwrap_or(&"Unknown".to_string()),
							fingerprint.0,
							e
						);
					}
				}
			}
		}

		info!("Loaded {} cloud volumes from database", loaded_count);
		Ok(())
	}

	/// Start background monitoring of volume changes
	pub async fn start_monitoring(&self) {
		if *self.is_monitoring.read().await {
			warn!("Volume monitoring already started");
			return;
		}

		*self.is_monitoring.write().await = true;

		// Start file system watcher for real-time detection
		self.start_volume_watcher().await;

		// Continue with existing timer-based monitoring as fallback
		let volumes = self.volumes.clone();
		let path_cache = self.path_cache.clone();
		let events = self.events.clone();
		let config = self.config.clone();
		let is_monitoring = self.is_monitoring.clone();
		let library_manager = self.library_manager.clone();
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
					&library_manager,
				)
				.await
				{
					error!("Error during volume refresh: {}", e);
				}
			}

			info!("Volume monitoring stopped");
		});
	}

	/// Start file system watcher for real-time volume change detection
	async fn start_volume_watcher(&self) {
		let watch_paths = get_volume_watch_paths();
		if watch_paths.is_empty() {
			debug!("No volume watch paths available on this platform, using timer-based monitoring only");
			return;
		}

		let volumes = self.volumes.clone();
		let path_cache = self.path_cache.clone();
		let events = self.events.clone();
		let config = self.config.clone();
		let library_manager = self.library_manager.clone();
		let device_id = self.device_id;
		let is_monitoring = self.is_monitoring.clone();

		// Create the watcher
		let (tx, mut rx) = tokio::sync::mpsc::channel(100);

		let watcher = notify::recommended_watcher(move |result: notify::Result<notify::Event>| {
			match result {
				Ok(event) => {
					// Send the event to our async handler
					if let Err(_) = tx.blocking_send(event) {
						// Channel closed, watcher is stopping
					}
				}
				Err(e) => {
					error!("Volume watcher error: {}", e);
				}
			}
		});

		match watcher {
			Ok(mut watcher) => {
				// Watch the volume directories
				for path in &watch_paths {
					if let Err(e) = watcher.watch(path, RecursiveMode::NonRecursive) {
						warn!("Failed to watch {}: {}", path.display(), e);
					} else {
						info!("Watching {} for volume changes", path.display());
					}
				}

				// Store the watcher
				*self.volume_watcher.write().await = Some(watcher);

				// Handle events
				tokio::spawn(async move {
					while let Some(event) = rx.recv().await {
						if !*is_monitoring.read().await {
							break;
						}

						// Check if this is a mount/unmount event
						if event.kind.is_create() || event.kind.is_remove() {
							debug!("Volume change detected: {:?}", event);

							// Debounce rapid events (reduced from 500ms for faster response)
							tokio::time::sleep(Duration::from_millis(200)).await;

							let start_time = std::time::Instant::now();

							// Trigger volume refresh
							match Self::refresh_volumes_internal(
								device_id,
								&volumes,
								&path_cache,
								&events,
								&config,
								&library_manager,
							)
							.await
							{
								Ok(()) => {
									let elapsed = start_time.elapsed();
									info!(
										"Event-triggered volume refresh completed in {:?}",
										elapsed
									);
								}
								Err(e) => {
									error!("Error during event-triggered volume refresh: {}", e);
								}
							}
						}
					}
					debug!("Volume watcher event handler stopped");
				});
			}
			Err(e) => {
				warn!("Failed to create volume watcher: {}", e);
			}
		}
	}

	/// Stop background monitoring
	pub async fn stop_monitoring(&self) {
		*self.is_monitoring.write().await = false;

		// Stop the file system watcher
		if let Some(_watcher) = self.volume_watcher.write().await.take() {
			debug!("Volume watcher stopped");
		}

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
			&self.library_manager,
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
		library_manager: &RwLock<Option<Weak<LibraryManager>>>,
	) -> VolumeResult<()> {
		debug!("Refreshing volumes for device {}", device_id);

		// Detect current volumes
		let detected_volumes = detection::detect_volumes(device_id, config).await?;
		debug!("VOLUME_DETECT: Detected {} volumes", detected_volumes.len());
		for vol in &detected_volumes {
			debug!(
				"VOLUME_DETECT: Found '{}' at {} - Type: {:?}, Auto-track: {}",
				vol.name,
				vol.mount_point.display(),
				vol.volume_type,
				vol.auto_track_eligible
			);
		}

		// Query database for tracked volumes to merge metadata
		let mut tracked_volumes_map: HashMap<VolumeFingerprint, (Uuid, Option<String>)> =
			HashMap::new();
		if let Some(lib_mgr) = library_manager.read().await.as_ref() {
			if let Some(lib_mgr) = lib_mgr.upgrade() {
				let libraries = lib_mgr.get_open_libraries().await;
				debug!("DB_MERGE: Found {} open libraries", libraries.len());
				for library in libraries {
					debug!(
						"DB_MERGE: Querying library {} for tracked volumes on device {}",
						library.id(),
						device_id
					);
					if let Ok(tracked_vols) = entities::volume::Entity::find()
						.filter(entities::volume::Column::DeviceId.eq(device_id))
						.all(library.db().conn())
						.await
					{
						debug!(
							"DB_MERGE: Found {} tracked volumes in library {}",
							tracked_vols.len(),
							library.id()
						);
						for db_vol in tracked_vols {
							let fingerprint = VolumeFingerprint(db_vol.fingerprint.clone());
							debug!("DB_MERGE: Found tracked volume - fingerprint: {}, display_name: {:?}",
								fingerprint.short_id(), db_vol.display_name);
							tracked_volumes_map
								.insert(fingerprint, (library.id(), db_vol.display_name));
						}
					} else {
						debug!(
							"DB_MERGE: Failed to query tracked volumes for library {}",
							library.id()
						);
					}
				}
			} else {
				debug!("DB_MERGE: Library manager weak reference could not be upgraded");
			}
		} else {
			debug!("DB_MERGE: No library manager reference available");
		}

		let mut current_volumes = volumes.write().await;
		let mut cache = path_cache.write().await;

		// Track which volumes we've seen in this refresh
		let mut seen_fingerprints = std::collections::HashSet::new();

		// Process detected volumes
		for mut detected in detected_volumes {
			let fingerprint = detected.fingerprint.clone();
			seen_fingerprints.insert(fingerprint.clone());

			// Merge tracked volume metadata from database
			if let Some((library_id, display_name)) = tracked_volumes_map.get(&fingerprint) {
				detected.is_tracked = true;
				detected.library_id = Some(*library_id);
				detected.display_name = display_name.clone();
			}

			debug!(
				"Processing volume '{}' with fingerprint {} (exists in cache: {}, is_tracked: {})",
				detected.name,
				fingerprint.short_id(),
				current_volumes.contains_key(&fingerprint),
				detected.is_tracked
			);

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
						current_volumes.insert(fingerprint.clone(), updated_volume.clone());

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

						// Emit ResourceChanged event for UI reactivity (only for user-visible volumes)
						if updated_volume.is_user_visible {
							use crate::domain::resource::EventEmitter;
							if let Err(e) = updated_volume.emit_changed(&events) {
								warn!("Failed to emit volume ResourceChanged: {}", e);
							}
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
					events.emit(Event::VolumeAdded(detected.clone()));

					// Emit ResourceChanged event for UI reactivity (only for user-visible volumes)
					if detected.is_user_visible {
						debug!(
							"Emitting ResourceChanged for user-visible volume: {} (is_user_visible={})",
							detected.name, detected.is_user_visible
						);
						use crate::domain::resource::EventEmitter;
						if let Err(e) = detected.emit_changed(&events) {
							warn!("Failed to emit volume ResourceChanged: {}", e);
						}
					} else {
						debug!(
							"Skipping ResourceChanged for non-user-visible volume: {} (is_user_visible={})",
							detected.name, detected.is_user_visible
						);
					}
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
				events.emit(Event::VolumeRemoved {
					fingerprint: fingerprint.clone(),
				});

				// Emit appropriate event based on tracking status
				if removed_volume.is_user_visible {
					use crate::domain::{resource::EventEmitter, Volume};

					if removed_volume.is_tracked {
						// Tracked volume - mark as offline but keep in UI
						let mut offline_volume = removed_volume.clone();
						offline_volume.is_mounted = false;

						if let Err(e) = offline_volume.emit_changed(&events) {
							warn!("Failed to emit volume ResourceChanged: {}", e);
						}
					} else {
						// Untracked volume - remove from UI
						Volume::emit_deleted(removed_volume.id, &events);
					}
				}
			}
		}

		// Update offline status for tracked volumes that are no longer detected
		// Note: This requires library context, so we'll add this to a separate method
		// that gets called from places where we have library access
		debug!(
			"Volume refresh completed. Detected {} volumes",
			seen_fingerprints.len()
		);

		Ok(())
	}

	/// Mark tracked volumes as offline if they're no longer detected
	/// This should be called after refresh_volumes_internal when we have library access
	pub async fn update_offline_volumes(
		&self,
		library: &crate::library::Library,
	) -> VolumeResult<()> {
		let db = library.db().conn();
		let current_volumes = self.volumes.read().await;
		let detected_fingerprints: std::collections::HashSet<_> =
			current_volumes.keys().cloned().collect();

		// Get all tracked volumes for this device
		let tracked_volumes = entities::volume::Entity::find()
			.filter(entities::volume::Column::DeviceId.eq(self.device_id))
			.all(db)
			.await
			.map_err(|e| VolumeError::Database(e.to_string()))?;

		let mut updated_count = 0;

		for tracked_volume in tracked_volumes {
			let fingerprint = VolumeFingerprint(tracked_volume.fingerprint.clone());
			let is_currently_detected = detected_fingerprints.contains(&fingerprint);

			// Update online status if it has changed
			if tracked_volume.is_online != is_currently_detected {
				let mut active_model: entities::volume::ActiveModel = tracked_volume.into();
				active_model.is_online = Set(is_currently_detected);
				active_model.last_seen_at = Set(chrono::Utc::now());

				active_model
					.update(db)
					.await
					.map_err(|e| VolumeError::Database(e.to_string()))?;

				updated_count += 1;

				if is_currently_detected {
					debug!("Marked volume {} as online", fingerprint.0);
				} else {
					debug!("Marked volume {} as offline", fingerprint.0);
				}
			}
		}

		if updated_count > 0 {
			debug!(
				"Updated online status for {} tracked volumes",
				updated_count
			);
		}

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

	/// Resolve a volume for an SdPath (unified method for cloud and local paths)
	/// This abstracts away the cloud/local path distinction
	pub async fn resolve_volume_for_sdpath(
		&self,
		sdpath: &crate::domain::addressing::SdPath,
		_library: &crate::library::Library,
	) -> VolumeResult<Option<Volume>> {
		// Check if this is a cloud path
		if let Some((service, identifier, _path)) = sdpath.as_cloud() {
			// Cloud path - use identity-based lookup
			Ok(self.find_cloud_volume(service, identifier).await)
		} else {
			// Local path - resolve by filesystem path
			if let Some(local_path) = sdpath.as_local_path() {
				Ok(self.volume_for_path(local_path).await)
			} else {
				Ok(None)
			}
		}
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

	/// Check if two paths are on the same physical storage (filesystem-aware)
	/// This is the enhanced version that uses filesystem-specific handlers
	pub async fn same_physical_storage(&self, path1: &Path, path2: &Path) -> bool {
		// 1. Get volumes for both paths
		let vol1 = self.volume_for_path(path1).await;
		let vol2 = self.volume_for_path(path2).await;

		match (&vol1, &vol2) {
			(Some(v1), Some(v2)) => {
				// 2. Check if same volume first (fast path)
				if v1.fingerprint == v2.fingerprint {
					debug!("Paths are on the same volume: {}", v1.fingerprint);
					return true;
				}

				// 3. Use filesystem-specific logic for cross-volume checks
				if v1.file_system == v2.file_system {
					debug!(
						"Using filesystem-specific handler for {} to check paths: {} vs {}",
						v1.file_system,
						path1.display(),
						path2.display()
					);
					return crate::volume::fs::same_physical_storage(path1, path2, &v1.file_system)
						.await;
				}

				debug!(
					"Different filesystems: {} vs {}",
					v1.file_system, v2.file_system
				);
				false
			}
			_ => {
				debug!(
					"Could not find volumes for paths: {:?}, {:?}",
					vol1.is_some(),
					vol2.is_some()
				);
				false
			}
		}
	}

	/// Get or initialize the I/O backend for a volume
	///
	/// This lazily initializes the backend on first access. Local volumes get a
	/// LocalBackend pointing to their mount point. Cloud volumes should have their
	/// backend set during creation.
	pub(crate) fn backend_for_volume(
		&self,
		volume: &mut Volume,
	) -> Arc<dyn crate::volume::VolumeBackend> {
		if let Some(backend) = &volume.backend {
			return backend.clone();
		}

		// Lazy-initialize LocalBackend for local volumes
		let backend: Arc<dyn crate::volume::VolumeBackend> =
			Arc::new(crate::volume::LocalBackend::new(&volume.mount_point));

		volume.backend = Some(backend.clone());
		backend
	}

	/// Find volumes with available space
	pub async fn volumes_with_space(&self, required_bytes: u64) -> Vec<Volume> {
		self.volumes
			.read()
			.await
			.values()
			.filter(|vol| vol.available_space >= required_bytes)
			.cloned()
			.collect()
	}

	/// Get volume statistics
	pub async fn get_statistics(&self) -> VolumeStatistics {
		let volumes = self.volumes.read().await;

		let total_volumes = volumes.len();
		let mounted_volumes = volumes.values().filter(|v| v.is_mounted).count();
		let total_capacity: u64 = volumes.values().map(|v| v.total_capacity).sum();
		let total_available: u64 = volumes.values().map(|v| v.available_space).sum();

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

	/// Find cloud volume by service type and identifier
	/// Uses mount point cache for O(1) lookup instead of scanning
	///
	/// # Examples
	/// ```ignore
	/// let volume = manager.find_cloud_volume(CloudServiceType::S3, "my-bucket").await;
	/// ```
	pub async fn find_cloud_volume(
		&self,
		service: crate::volume::backend::CloudServiceType,
		identifier: &str,
	) -> Option<Volume> {
		// Construct the mount point string (e.g., "s3://my-bucket")
		let mount_point_key = format!("{}://{}", service.scheme(), identifier);

		// Check cache first for O(1) lookup
		{
			let mount_point_cache = self.mount_point_cache.read().await;
			if let Some(fingerprint) = mount_point_cache.get(&mount_point_key) {
				let volumes = self.volumes.read().await;
				if let Some(volume) = volumes.get(fingerprint) {
					return Some(volume.clone());
				}
			}
		}

		// Cache miss - fall back to scanning (for volumes added before cache was implemented)
		let volumes = self.volumes.read().await;
		volumes.values().find_map(|volume| {
			if let Some((vol_service, vol_id)) = volume.parse_cloud_identity() {
				if vol_service == service && vol_id == identifier {
					// Update cache for next time
					let mount_point_key = format!("{}://{}", service.scheme(), identifier);
					let mut mount_point_cache = self.mount_point_cache.blocking_write();
					mount_point_cache.insert(mount_point_key, volume.fingerprint.clone());

					return Some(volume.clone());
				}
			}
			None
		})
	}

	/// Ensure mount point is unique by appending -2, -3, etc. if needed
	/// Used during volume creation to prevent collisions
	///
	/// # Examples
	/// ```ignore
	/// let mount_point = manager.ensure_unique_mount_point("s3://my-bucket").await;
	/// // Returns "s3://my-bucket" or "s3://my-bucket-2" if collision exists
	/// ```
	pub async fn ensure_unique_mount_point(&self, desired: &str) -> PathBuf {
		let volumes = self.volumes.read().await;

		let base = desired;
		let mut candidate = base.to_string();
		let mut counter = 2;

		while volumes
			.values()
			.any(|v| v.mount_point.to_string_lossy() == candidate)
		{
			candidate = format!("{}-{}", base, counter);
			counter += 1;
		}

		PathBuf::from(candidate)
	}

	/// Register a cloud volume with the volume manager
	/// This adds the volume to the internal volumes map so it can be tracked
	pub async fn register_cloud_volume(&self, volume: Volume) {
		let fingerprint = volume.fingerprint.clone();

		let mut volumes = self.volumes.write().await;

		info!(
			"Registering cloud volume '{}' with fingerprint {}",
			volume.name, fingerprint
		);

		// Update mount point cache for fast cloud volume lookup using cloud_identifier
		if let Some((service, identifier)) = volume.parse_cloud_identity() {
			let cache_key = format!("{}://{}", service.scheme(), identifier);
			let mut mount_point_cache = self.mount_point_cache.write().await;
			mount_point_cache.insert(cache_key, fingerprint.clone());
		}

		volumes.insert(fingerprint.clone(), volume);
	}

	/// Track a volume in the specified library
	pub async fn track_volume(
		&self,
		library: &crate::library::Library,
		fingerprint: &VolumeFingerprint,
		display_name: Option<String>,
	) -> VolumeResult<entities::volume::Model> {
		// Find the volume in our current detected volumes
		let volume = {
			let volumes = self.volumes.read().await;
			volumes
				.get(fingerprint)
				.cloned()
				.ok_or_else(|| VolumeError::NotFound(fingerprint.to_string()))?
		};

		// Try to create/read identifier file for this volume
		if let Some(spacedrive_id) = self.manage_spacedrive_identifier(&volume).await {
			info!(
				"Created/found Spacedrive ID {} for manually tracked volume {}",
				spacedrive_id, volume.name
			);

			// Check if we should upgrade to Spacedrive ID-based fingerprint
			let spacedrive_fingerprint = VolumeFingerprint::from_spacedrive_id(spacedrive_id);
			if spacedrive_fingerprint != volume.fingerprint {
				info!(
					"Upgrading fingerprint for volume {} from content-based to Spacedrive ID-based",
					volume.name
				);
				// Note: In a full implementation, we'd want to update the volume's fingerprint
				// and potentially migrate database records. For now, we'll log this.
			}
		}

		// Check if volume is already tracked
		if let Some(existing) = entities::volume::Entity::find()
			.filter(entities::volume::Column::Fingerprint.eq(fingerprint.0.clone()))
			.filter(entities::volume::Column::DeviceId.eq(volume.device_id))
			.one(library.db().conn())
			.await
			.map_err(|e| VolumeError::Database(e.to_string()))?
		{
			warn!(
				"Volume {} is already tracked in library {}",
				volume.name,
				library.name().await
			);
			return Ok(existing);
		}

		// Determine removability and network status
		let is_removable = matches!(volume.mount_type, crate::volume::types::MountType::External);
		let is_network_drive =
			matches!(volume.mount_type, crate::volume::types::MountType::Network);

		// Determine final display name (fallback to volume's name if not provided)
		let final_display_name = display_name
			.or(volume.display_name.clone())
			.or(Some(volume.name.clone()));

		// Create tracking record
		let active_model = entities::volume::ActiveModel {
			uuid: Set(volume.id),             // Use the volume's UUID
			device_id: Set(volume.device_id), // Use Uuid directly
			fingerprint: Set(fingerprint.0.clone()),
			display_name: Set(final_display_name.clone()),
			tracked_at: Set(chrono::Utc::now()),
			last_seen_at: Set(chrono::Utc::now()),
			is_online: Set(volume.is_mounted),
			total_capacity: Set(Some(volume.total_capacity as i64)),
			available_capacity: Set(Some(volume.available_space as i64)),
			read_speed_mbps: Set(volume.read_speed_mbps.map(|s| s as i32)),
			write_speed_mbps: Set(volume.write_speed_mbps.map(|s| s as i32)),
			last_speed_test_at: Set(None),
			file_system: Set(Some(volume.file_system.to_string())),
			mount_point: Set(Some(volume.mount_point.to_string_lossy().to_string())),
			is_removable: Set(Some(is_removable)),
			is_network_drive: Set(Some(is_network_drive)),
			device_model: Set(volume.hardware_id.clone()),
			// Save volume classification fields
			volume_type: Set(Some(format!("{:?}", volume.volume_type))),
			is_user_visible: Set(Some(volume.is_user_visible)),
			auto_track_eligible: Set(Some(volume.auto_track_eligible)),
			cloud_identifier: Set(volume.cloud_identifier.clone()),
			cloud_config: Set(volume.cloud_config.as_ref().map(|c| c.to_string())),
			..Default::default()
		};

		let model = active_model
			.insert(library.db().conn())
			.await
			.map_err(|e| VolumeError::Database(e.to_string()))?;

		info!(
			"Tracked volume '{}' for library '{}'",
			final_display_name.as_ref().unwrap_or(&volume.name),
			library.name().await
		);

		// Emit tracking event
		self.events.emit(Event::Custom {
			event_type: "VolumeTracked".to_string(),
			data: serde_json::json!({
				"library_id": library.id(),
				"volume_fingerprint": fingerprint.to_string(),
				"display_name": final_display_name,
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
		active_model.total_capacity = Set(Some(volume.total_capacity as i64));
		active_model.available_capacity = Set(Some(volume.available_space as i64));

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

	/// Auto-track eligible volumes (only Primary system volume)
	pub async fn auto_track_user_volumes(
		&self,
		library: &crate::library::Library,
	) -> VolumeResult<Vec<entities::volume::Model>> {
		let all_volumes = self.volumes.read().await;
		let total_count = all_volumes.len();

		debug!("AUTO_TRACK: Total volumes detected: {}", total_count);
		for (fp, vol) in all_volumes.iter() {
			debug!(
				"AUTO_TRACK: Volume '{}' - Type: {:?}, Eligible: {}, Fingerprint: {}",
				vol.name, vol.volume_type, vol.auto_track_eligible, fp
			);
		}

		let eligible_volumes: Vec<_> = all_volumes
			.values()
			.filter(|v| v.auto_track_eligible)
			.cloned()
			.collect();
		drop(all_volumes);

		debug!(
			"AUTO_TRACK: Eligible volumes for tracking: {}",
			eligible_volumes.len()
		);
		let mut tracked_volumes = Vec::new();

		for volume in eligible_volumes {
			// Try to create/read identifier file for better fingerprinting
			if let Some(spacedrive_id) = self.manage_spacedrive_identifier(&volume).await {
				info!(
					"Using Spacedrive ID {} for volume {} fingerprinting",
					spacedrive_id, volume.name
				);
				// We could update the fingerprint here, but for now we'll keep using the existing one
				// to maintain compatibility with already tracked volumes
			}

			if !self.is_volume_tracked(library, &volume.fingerprint).await? {
				// Use display_name if available, otherwise fall back to name
				let display_name = volume
					.display_name
					.clone()
					.unwrap_or_else(|| volume.name.clone());

				match self
					.track_volume(library, &volume.fingerprint, Some(display_name.clone()))
					.await
				{
					Ok(tracked_volume) => {
						info!("Auto-tracked volume: {}", display_name);
						tracked_volumes.push(tracked_volume);
					}
					Err(e) => {
						warn!("Failed to auto-track volume {}: {}", display_name, e);
					}
				}
			}
		}

		Ok(tracked_volumes)
	}

	/// Automatically track system volumes for a library (legacy - use auto_track_user_volumes instead)
	pub async fn auto_track_system_volumes(
		&self,
		library: &crate::library::Library,
	) -> VolumeResult<Vec<entities::volume::Model>> {
		// Use the new filtered auto-tracking
		self.auto_track_user_volumes(library).await
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

	/// Calculate and save unique bytes for a specific volume (owned by this device)
	/// This deduplicates content using content_identity hashes
	pub async fn calculate_and_save_unique_bytes(
		&self,
		fingerprint: &VolumeFingerprint,
		libraries: &[Arc<crate::library::Library>],
	) -> VolumeResult<()> {
		use sea_orm::{DbBackend, FromQueryResult, Statement};

		for library in libraries {
			// Check if this volume is tracked in this library
			if self.is_volume_tracked(library, fingerprint).await? {
				let db = library.db().conn();

				// Get the volume from database to get mount_point and verify ownership
				let db_volume = entities::volume::Entity::find()
					.filter(entities::volume::Column::DeviceId.eq(self.device_id))
					.filter(entities::volume::Column::Fingerprint.eq(fingerprint.0.clone()))
					.one(db)
					.await
					.map_err(|e| VolumeError::Database(e.to_string()))?;

				let db_volume = match db_volume {
					Some(v) => v,
					None => {
						debug!(
							"Volume {} not found or not owned by this device, skipping unique_bytes calculation",
							fingerprint.0
						);
						continue;
					}
				};

				let mount_point = match &db_volume.mount_point {
					Some(mp) => mp,
					None => {
						debug!(
							"Volume {} has no mount point, cannot calculate unique_bytes",
							fingerprint.0
						);
						continue;
					}
				};

				info!(
					"Calculating unique bytes for volume {} in library {}",
					fingerprint.0,
					library.name().await
				);

				// Calculate unique bytes using content_identity deduplication
				let query = r#"
					SELECT COALESCE(SUM(unique_size), 0) as unique_bytes
					FROM (
						SELECT ci.content_hash, ci.total_size as unique_size
						FROM entries e
						INNER JOIN directory_paths dp ON e.id = dp.entry_id
						INNER JOIN content_identities ci ON e.content_id = ci.id
						WHERE dp.path LIKE ? || '%'
						  AND e.kind = 0
						GROUP BY ci.content_hash, ci.total_size
					)
				"#;

				#[derive(FromQueryResult)]
				struct UniqueResult {
					unique_bytes: i64,
				}

				let result = UniqueResult::find_by_statement(Statement::from_sql_and_values(
					DbBackend::Sqlite,
					query,
					vec![mount_point.clone().into()],
				))
				.one(db)
				.await
				.map_err(|e| VolumeError::Database(e.to_string()))?;

				let unique_bytes = result.map(|r| r.unique_bytes).unwrap_or(0);

				// Update the volume record with calculated unique_bytes
				let update_result = entities::volume::Entity::update_many()
					.filter(entities::volume::Column::DeviceId.eq(self.device_id))
					.filter(entities::volume::Column::Fingerprint.eq(fingerprint.0.clone()))
					.set(entities::volume::ActiveModel {
						unique_bytes: Set(Some(unique_bytes)),
						..Default::default()
					})
					.exec(db)
					.await
					.map_err(|e| VolumeError::Database(e.to_string()))?;

				if update_result.rows_affected > 0 {
					info!(
						"Saved unique_bytes for volume {} in library {}: {} bytes ({:.2} GB)",
						fingerprint.0,
						library.name().await,
						unique_bytes,
						unique_bytes as f64 / 1_073_741_824.0
					);
				}
			}
		}

		Ok(())
	}

	/// Calculate and save unique bytes for all volumes owned by this device
	pub async fn calculate_unique_bytes_for_owned_volumes(
		&self,
		libraries: &[Arc<crate::library::Library>],
	) -> VolumeResult<()> {
		info!(
			"Calculating unique bytes for all volumes owned by device {}",
			self.device_id
		);

		for library in libraries {
			let db = library.db().conn();

			// Get all tracked volumes owned by this device
			let owned_volumes = entities::volume::Entity::find()
				.filter(entities::volume::Column::DeviceId.eq(self.device_id))
				.all(db)
				.await
				.map_err(|e| VolumeError::Database(e.to_string()))?;

			info!(
				"Found {} volumes owned by this device in library {}",
				owned_volumes.len(),
				library.name().await
			);

			for volume in owned_volumes {
				let fingerprint = VolumeFingerprint(volume.fingerprint.clone());

				// Calculate and save unique bytes for this volume
				if let Err(e) = self
					.calculate_and_save_unique_bytes(&fingerprint, &[library.clone()])
					.await
				{
					warn!(
						"Failed to calculate unique_bytes for volume {}: {}",
						fingerprint.0, e
					);
					// Continue with other volumes even if one fails
				}
			}
		}

		Ok(())
	}

	/// Get volume by short ID
	pub async fn get_volume_by_short_id(&self, short_id: &str) -> Option<Volume> {
		if !VolumeFingerprint::is_short_id(short_id) && !VolumeFingerprint::is_medium_id(short_id) {
			return None;
		}

		let volumes = self.volumes.read().await;
		for volume in volumes.values() {
			if volume.fingerprint.matches_short_id(short_id) {
				return Some(volume.clone());
			}
		}
		None
	}

	/// Get volumes that match a partial name (for smart name matching)
	pub async fn get_volumes_by_name(&self, name: &str) -> Vec<Volume> {
		let volumes = self.volumes.read().await;
		let name_lower = name.to_lowercase();

		volumes
			.values()
			.filter(|volume| volume.name.to_lowercase().contains(&name_lower))
			.cloned()
			.collect()
	}

	/// Create or read Spacedrive identifier file for a volume
	/// Returns the UUID from the identifier file if successfully created/read
	async fn manage_spacedrive_identifier(&self, volume: &Volume) -> Option<Uuid> {
		// Skip cloud volumes - they don't have filesystem mount points
		if matches!(
			volume.volume_type,
			crate::volume::types::VolumeType::Network
		) && matches!(volume.mount_type, crate::volume::types::MountType::Network)
		{
			debug!(
				"Skipping Spacedrive identifier management for cloud volume: {}",
				volume.name
			);
			return None;
		}

		let id_file_path = volume.mount_point.join(SPACEDRIVE_VOLUME_ID_FILE);

		// Try to read existing identifier file
		if let Ok(content) = fs::read_to_string(&id_file_path).await {
			if let Ok(spacedrive_id) = serde_json::from_str::<SpacedriveVolumeId>(&content) {
				debug!(
					"Found existing Spacedrive ID {} for volume {}",
					spacedrive_id.id, volume.name
				);
				return Some(spacedrive_id.id);
			}
		}

		// Try to create new identifier file if volume is writable
		if !volume.is_read_only && volume.mount_point.exists() {
			let spacedrive_id = SpacedriveVolumeId {
				id: Uuid::new_v4(),
				created: chrono::Utc::now(),
				device_name: None, // TODO: Get from DeviceManager when available
				volume_name: volume.name.clone(),
				device_id: volume.device_id,
				library_id: Uuid::nil(), // TODO: Populate from library context when available
			};

			if let Ok(content) = serde_json::to_string_pretty(&spacedrive_id) {
				match fs::write(&id_file_path, content).await {
					Ok(()) => {
						info!(
							"Created Spacedrive ID {} for volume {} at {}",
							spacedrive_id.id,
							volume.name,
							id_file_path.display()
						);
						return Some(spacedrive_id.id);
					}
					Err(e) => {
						debug!(
							"Failed to write Spacedrive ID file to {}: {}",
							id_file_path.display(),
							e
						);
					}
				}
			}
		}

		debug!(
			"Could not create or read Spacedrive identifier for volume {} (read_only: {}, exists: {})",
			volume.name,
			volume.is_read_only,
			volume.mount_point.exists()
		);
		None
	}

	/// Read Spacedrive identifier file from a volume if it exists
	pub async fn read_spacedrive_identifier(
		&self,
		mount_point: &Path,
	) -> Option<SpacedriveVolumeId> {
		let id_file_path = mount_point.join(SPACEDRIVE_VOLUME_ID_FILE);

		if let Ok(content) = fs::read_to_string(&id_file_path).await {
			if let Ok(spacedrive_id) = serde_json::from_str::<SpacedriveVolumeId>(&content) {
				return Some(spacedrive_id);
			}
		}

		None
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
