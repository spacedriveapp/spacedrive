//! Volume monitoring service
//!
//! Periodically refreshes volume information and updates tracked volumes in the database.

use crate::{
	context::CoreContext, infra::event::EventBus, library::LibraryManager, service::Service,
	volume::VolumeManager,
};
use anyhow::Result;
use std::sync::{Arc, Weak};
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Configuration for volume monitoring
#[derive(Debug, Clone)]
pub struct VolumeMonitorConfig {
	/// How often to refresh volume information (in seconds)
	pub refresh_interval_secs: u64,
	/// Whether to update tracked volumes in the database
	pub update_tracked_volumes: bool,
}

impl Default for VolumeMonitorConfig {
	fn default() -> Self {
		Self {
			refresh_interval_secs: 30,
			update_tracked_volumes: true,
		}
	}
}

/// Background service that monitors volume state changes
pub struct VolumeMonitorService {
	volume_manager: Arc<VolumeManager>,
	library_manager: Weak<LibraryManager>,
	config: VolumeMonitorConfig,
	running: RwLock<bool>,
	handle: RwLock<Option<tokio::task::JoinHandle<()>>>,
}

impl VolumeMonitorService {
	/// Create a new volume monitor service
	pub fn new(
		volume_manager: Arc<VolumeManager>,
		library_manager: Weak<LibraryManager>,
		config: VolumeMonitorConfig,
	) -> Self {
		Self {
			volume_manager,
			library_manager,
			config,
			running: RwLock::new(false),
			handle: RwLock::new(None),
		}
	}

	/// Bring a library's tracked volume rows in line with what detection
	/// returns right now.
	///
	/// A tracked volume detection still returns follows its mount state. One
	/// detection no longer returns is marked offline: a drive in a drawer or a
	/// dataset whose key is not loaded does not stay online because its row
	/// said so at the last refresh. Every change is also told to the volume
	/// index, so the sources on that drive detach or attach without waiting
	/// for a restart.
	pub async fn reconcile_tracked_volumes(
		volume_manager: &VolumeManager,
		library: &Arc<crate::library::Library>,
	) -> Result<()> {
		let volume_index = library.core_context().volume_index();
		let tracked_volumes = volume_manager.get_tracked_volumes(library).await?;
		// The table holds every device's rows; a peer's drive is not mounted
		// here and its row is not this device's to write.
		for tracked in tracked_volumes
			.into_iter()
			.filter(|tracked| tracked.device_id == volume_manager.device_id)
		{
			let current = volume_manager.get_volume(&tracked.fingerprint).await;
			let mounted = current.as_ref().is_some_and(|volume| volume.is_mounted);
			if tracked.is_online == mounted {
				continue;
			}

			// One row that cannot be written must not keep the others from
			// their transition, so each failure is logged and the pass goes on.
			let written = match &current {
				Some(volume) => {
					volume_manager
						.update_tracked_volume_state(library, &tracked.fingerprint, volume)
						.await
				}
				None => {
					volume_manager
						.mark_tracked_volume_offline(library, &tracked.fingerprint)
						.await
				}
			};
			if let Err(e) = written {
				error!(
					"Failed to update tracked volume {} in library {}: {}",
					tracked.fingerprint,
					library.id(),
					e
				);
				continue;
			}

			let mount_point = current
				.as_ref()
				.map(|volume| volume.mount_point.clone())
				.unwrap_or_else(|| {
					std::path::PathBuf::from(tracked.mount_point.as_deref().unwrap_or_default())
				});
			volume_index.set_volume_mounted(tracked.uuid, &mount_point, mounted);
			info!(
				"Tracked volume {} in library {} is now {}",
				tracked.fingerprint,
				library.id(),
				if mounted { "online" } else { "offline" }
			);
		}
		Ok(())
	}

	/// Monitor volumes and update tracked volumes in libraries
	async fn monitor_loop(
		volume_manager: Arc<VolumeManager>,
		library_manager: Weak<LibraryManager>,
		config: VolumeMonitorConfig,
		running: Arc<RwLock<bool>>,
	) {
		let mut interval = tokio::time::interval(Duration::from_secs(config.refresh_interval_secs));

		while *running.read().await {
			interval.tick().await;

			// Refresh all volumes
			if let Err(e) = volume_manager.refresh_volumes().await {
				error!("Failed to refresh volumes: {}", e);
				continue;
			}

			// Update tracked volumes if enabled and library manager is available
			if config.update_tracked_volumes {
				if let Some(lib_manager) = library_manager.upgrade() {
					debug!("Updating tracked volumes across libraries");

					// Get all open libraries
					let libraries = lib_manager.get_open_libraries().await;

					for library in &libraries {
						if let Err(e) =
							Self::reconcile_tracked_volumes(&volume_manager, library).await
						{
							error!(
								"Failed to reconcile tracked volumes for library {}: {}",
								library.id(),
								e
							);
						}
					}

					// Check for new external volumes to auto-track
					let all_volumes = volume_manager.get_all_volumes().await;
					for volume in all_volumes {
						// Only consider external volumes
						if matches!(volume.mount_type, crate::volume::types::MountType::External) {
							for library in &libraries {
								// Check if auto-tracking is enabled
								let config = library.config().await;
								if config.settings.auto_track_external_volumes {
									// Check if not already tracked
									if !volume_manager
										.is_volume_tracked(&library, &volume.fingerprint)
										.await
										.unwrap_or(false)
									{
										// Auto-track the external volume
										match volume_manager
											.track_volume(&library, &volume.fingerprint, None)
											.await
										{
											Ok(_) => {
												info!(
                                                    "Auto-tracked external volume '{}' in library '{}'",
                                                    volume.name,
                                                    library.name().await
                                                );
											}
											Err(e) => {
												debug!(
													"Failed to auto-track external volume '{}': {}",
													volume.name, e
												);
											}
										}
									}
								}
							}
						}
					}
				} else {
					debug!("Library manager not available, skipping tracked volume updates");
				}
			}
		}

		info!("Volume monitoring stopped");
	}
}

#[async_trait::async_trait]
impl Service for VolumeMonitorService {
	async fn start(&self) -> Result<()> {
		let mut running = self.running.write().await;
		if *running {
			warn!("Volume monitor service already running");
			return Ok(());
		}

		*running = true;

		let volume_manager = self.volume_manager.clone();
		let library_manager = self.library_manager.clone();
		let config = self.config.clone();
		let running_flag = Arc::new(RwLock::new(*running));

		let handle = tokio::spawn(Self::monitor_loop(
			volume_manager,
			library_manager,
			config,
			running_flag,
		));

		*self.handle.write().await = Some(handle);

		info!(
			"Volume monitor service started (refresh every {}s)",
			self.config.refresh_interval_secs
		);

		Ok(())
	}

	async fn stop(&self) -> Result<()> {
		*self.running.write().await = false;

		if let Some(handle) = self.handle.write().await.take() {
			handle.abort();
		}

		info!("Volume monitor service stopped");
		Ok(())
	}

	fn is_running(&self) -> bool {
		// Use blocking read since this is a sync method
		*self.running.blocking_read()
	}

	fn name(&self) -> &'static str {
		"volume_monitor"
	}
}
