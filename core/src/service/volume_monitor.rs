//! Volume monitoring service
//!
//! Periodically refreshes volume information and updates tracked volumes in
//! the database. The volume index does not wait for this loop: it follows
//! the volume manager's events through [`follow_volume_events`], so a
//! source detaches or reattaches as soon as a refresh notices its drive.

use crate::{
	context::CoreContext,
	infra::event::{Event, EventBus},
	library::LibraryManager,
	service::Service,
	volume::{Volume, VolumeFingerprint, VolumeManager, VolumeState},
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
	/// said so at the last refresh. The volume index learns the same change
	/// from the manager's events, not from here.
	pub async fn reconcile_tracked_volumes(
		volume_manager: &VolumeManager,
		library: &Arc<crate::library::Library>,
	) -> Result<()> {
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

			info!(
				"Tracked volume {} in library {} is now {}",
				tracked.fingerprint,
				library.id(),
				current
					.as_ref()
					.map(|volume| volume.state())
					.unwrap_or(VolumeState::Unmounted)
					.as_str()
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
						// Only consider mounted external volumes; a dataset that
						// is away cannot carry an identity file yet.
						if volume.is_mounted
							&& matches!(
								volume.mount_type,
								crate::volume::types::MountType::External
							) {
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

/// Keep the volume index in step with the volume manager for as long as the
/// core runs.
///
/// The manager emits an event when a drive appears, disappears, or changes
/// mount state, and the index is the one place that knows which sources sit
/// on that drive. Routing the change through the event bus rather than the
/// monitor's loop means a refresh triggered by the mount watcher, by the
/// `volumes.refresh` op or by the timer all reach the index the same way.
/// When a drive returns, every source on it that had failed identifications
/// while it was away has them put back in the pending set and a background
/// identity pass dispatched, so a file that could not be read because its
/// volume was locked is read once the key loads.
pub fn follow_volume_events(context: Arc<CoreContext>) -> tokio::task::JoinHandle<()> {
	let mut events = context.events.subscribe();
	tokio::spawn(async move {
		loop {
			let event = match events.recv().await {
				Ok(event) => event,
				Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
					// A removal is emitted once; a drive whose removal fell in
					// the gap would read mounted for the rest of the session,
					// so the index is re-read against the manager instead.
					warn!("volume follower skipped {skipped} events; resyncing the index");
					resync(&context).await;
					continue;
				}
				Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
			};
			match event {
				Event::VolumeAdded(volume) => apply_volume(&context, &volume).await,
				Event::VolumeMountChanged { fingerprint, .. }
				| Event::VolumeUpdated { fingerprint, .. } => {
					if let Some(volume) = context.volume_manager.get_volume(&fingerprint).await {
						apply_volume(&context, &volume).await;
					}
				}
				Event::VolumeRemoved { fingerprint } => vanished(&context, &fingerprint).await,
				_ => {}
			}
		}
	})
}

/// Bring every drive the index maps in line with what the manager holds now,
/// for a window of events the follower did not see.
async fn resync(context: &Arc<CoreContext>) {
	let live = context.volume_manager.get_all_volumes().await;
	for (uuid, fingerprint) in context.volume_index().mapped_volumes() {
		let Some(fingerprint) = fingerprint else {
			continue;
		};
		match live.iter().find(|volume| volume.fingerprint == fingerprint) {
			Some(volume) => apply_volume(context, volume).await,
			None if context.volume_index().volume_state(uuid) != Some(VolumeState::Unmounted) => {
				vanished(context, &fingerprint).await
			}
			None => {}
		}
	}
}

/// A drive detection returned; the index follows its state when it maps
/// the drive. `volume.id` is the row uuid once the drive is tracked by a
/// library, and the fingerprint reaches a drive tracked before that.
async fn apply_volume(context: &Arc<CoreContext>, volume: &Volume) {
	let index = context.volume_index();
	let uuid = match index.volume_by_fingerprint(&volume.fingerprint) {
		Some((uuid, _)) => uuid,
		None if index.volume_state(volume.id).is_some() => volume.id,
		None => return,
	};
	let state = volume.state();
	if index.volume_state(uuid) == Some(state) {
		// Capacity figures change every refresh; only a state change moves
		// anything here.
		return;
	}
	let returned = index
		.volume_state_changed(uuid, &volume.mount_point, state)
		.await;
	info!(
		volume = %uuid,
		mount_point = %volume.mount_point.display(),
		state = state.as_str(),
		"volume index followed a mount change"
	);
	if returned.is_empty() {
		return;
	}
	let libraries = context.libraries().await;
	for (library_id, source_id, root) in returned {
		let Some(library_id) = library_id else {
			continue;
		};
		let Some(library) = libraries.get_library(library_id).await else {
			continue;
		};
		let Some(store) = index.store_for(&root).await else {
			continue;
		};
		match store.retry_failed_identifications().await {
			Ok(0) => {}
			Ok(reset) => {
				info!(source = %source_id, reset, "failed identifications are pending again")
			}
			Err(error) => {
				warn!(source = %source_id, %error, "could not reset failed identifications")
			}
		}
		let pending = store.files_needing_content_count().await.unwrap_or(0);
		if pending == 0 {
			continue;
		}
		if let Some(reason) = index.dispatch_refusal(&root) {
			warn!(source = %source_id, %reason, "not identifying the returned source");
			continue;
		}
		let job =
			crate::ops::indexing::content_identity::ContentIdentityJob::background(root.clone());
		match library
			.jobs()
			.dispatch_with_priority(job, crate::infra::job::types::JobPriority::LOW, None)
			.await
		{
			Ok(handle) => info!(
				source = %source_id,
				pending,
				job = %handle.id(),
				"identifying the contents of a returned source"
			),
			Err(error) => warn!(source = %source_id, %error, "could not resume identification"),
		}
	}
}

/// Detection stopped returning a drive: its sources detach at the mount
/// point they had, so their maps stay readable there.
async fn vanished(context: &Arc<CoreContext>, fingerprint: &VolumeFingerprint) {
	let index = context.volume_index();
	let Some((uuid, mount_point)) = index.volume_by_fingerprint(fingerprint) else {
		return;
	};
	if index.volume_state(uuid) == Some(VolumeState::Unmounted) {
		return;
	}
	index
		.volume_state_changed(uuid, &mount_point, VolumeState::Unmounted)
		.await;
	info!(volume = %uuid, mount_point = %mount_point.display(), "volume index followed a vanished drive");
}
