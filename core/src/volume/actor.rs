use super::{
	error::VolumeError,
	speed::SpeedTest,
	types::{Volume, VolumeEvent, VolumeOptions},
	volumes::Volumes,
	watcher::VolumeWatcher,
	VolumeManagerContext, VolumeManagerState,
};
use crate::volume::types::VolumeFingerprint;
use crate::{
	library::{Library, LibraryManagerEvent},
	volume::MountType,
};
use async_channel as chan;
use sd_core_sync::DevicePubId;
use sd_prisma::prisma::volume;
use std::{sync::Arc, time::Duration};
use tokio::sync::{broadcast, oneshot, Mutex, RwLock};
use tracing::{debug, error, info, trace, warn};

const DEFAULT_CHANNEL_SIZE: usize = 128;

#[derive(Debug)]
pub enum VolumeManagerMessage {
	TrackVolume {
		fingerprint: VolumeFingerprint,
		library: Arc<Library>,
		ack: oneshot::Sender<Result<(), VolumeError>>,
	},
	UntrackVolume {
		fingerprint: VolumeFingerprint,
		library: Arc<Library>,
		ack: oneshot::Sender<Result<(), VolumeError>>,
	},
	UpdateVolume {
		volume: Volume,
		ack: oneshot::Sender<Result<(), VolumeError>>,
	},
	MountVolume {
		fingerprint: VolumeFingerprint,
		ack: oneshot::Sender<Result<(), VolumeError>>,
	},
	UnmountVolume {
		fingerprint: VolumeFingerprint,
		ack: oneshot::Sender<Result<(), VolumeError>>,
	},
	SpeedTest {
		fingerprint: VolumeFingerprint,
		library: Arc<Library>,
		ack: oneshot::Sender<Result<(), VolumeError>>,
	},
	ListSystemVolumes {
		library: Arc<Library>,
		ack: oneshot::Sender<Result<Vec<Volume>, VolumeError>>,
	},
	ListLibraryVolumes {
		library: Arc<Library>,
		ack: oneshot::Sender<Result<Vec<Volume>, VolumeError>>,
	},
}

#[derive(Clone)]
pub struct VolumeManagerActor {
	state: Arc<RwLock<VolumeManagerState>>,
	message_rx: chan::Receiver<VolumeManagerMessage>,
	event_tx: broadcast::Sender<VolumeEvent>,
	ctx: Arc<VolumeManagerContext>,
}

impl VolumeManagerActor {
	pub async fn new(ctx: Arc<VolumeManagerContext>) -> Result<(Volumes, Self), VolumeError> {
		Self::new_with_config(ctx, VolumeOptions::default()).await
	}

	// Creates a new VolumeManagerActor with custom configuration
	pub async fn new_with_config(
		ctx: Arc<VolumeManagerContext>,
		options: VolumeOptions,
	) -> Result<(Volumes, Self), VolumeError> {
		let (message_tx, message_rx) = chan::bounded(DEFAULT_CHANNEL_SIZE);
		let (event_tx, _) = broadcast::channel(DEFAULT_CHANNEL_SIZE);

		let manager = Volumes::new(message_tx, event_tx.clone());
		let state =
			VolumeManagerState::new(ctx.device_id.clone().into(), options, event_tx.clone());

		let actor = VolumeManagerActor {
			state: Arc::new(RwLock::new(state)),
			message_rx,
			event_tx,
			ctx,
		};

		Ok((manager, actor))
	}

	/// Starts the VolumeManagerActor
	/// It will scan volumes, start the watcher, start the maintenance task, and handle messages
	pub async fn start(self, device_id: DevicePubId) {
		info!("Volume manager actor started");
		let self_arc = Arc::new(Mutex::new(self));

		// Start event monitoring
		let actor = self_arc.lock().await;
		let event_rx = actor.event_tx.subscribe();
		actor
			.clone()
			.start_event_monitoring(event_rx, device_id.clone());
		drop(actor);

		// Handle messages
		let self_arc_msg = Arc::clone(&self_arc);
		tokio::spawn(async move {
			let message_rx = self_arc_msg.lock().await.message_rx.clone();
			while let Ok(msg) = message_rx.recv().await {
				let self_arc_inner = Arc::clone(&self_arc_msg);
				if let Err(e) = {
					let mut actor = self_arc_inner.lock().await;
					actor.handle_message(msg).await
				} {
					error!(?e, "Error handling volume manager message");
				}
			}
		});

		// Scan for volumes on startup
		// unlock registry rwlock
		let _ = self_arc.lock().await.scan_volumes().await;

		// Subscribe to LibraryManagerEvent
		let self_arc_subscribe = Arc::clone(&self_arc);
		let rx = {
			let actor = self_arc.lock().await;
			actor.ctx.library_event_tx.clone()
		};
		// This is a fire-and-forget subscription
		tokio::spawn(async move {
			if let Err(e) = rx
				.subscribe(move |event| {
					let self_arc_inner = Arc::clone(&self_arc_subscribe);
					async move {
						match event {
							LibraryManagerEvent::Load(library) => {
								if let Err(e) = {
									let mut actor = self_arc_inner.lock().await;

									actor.initialize_for_library(library.clone()).await
								} {
									error!(?e, "Failed to initialize volume manager for library");
								}
							}
							LibraryManagerEvent::Delete(library) => {
								if let Err(e) = {
									let mut actor = self_arc_inner.lock().await;
									actor.handle_library_deletion(library).await
								} {
									error!(?e, "Failed to cleanup library volumes");
								}
							}
							_ => {}
						}
					}
				})
				.await
			{
				error!(?e, "VolumeManager's library subscription failed");
			}
		});

		let event_tx = self_arc.lock().await.event_tx.clone();

		// Start the volume watcher
		let self_arc_watcher = Arc::clone(&self_arc);
		tokio::spawn(async move {
			let watcher = VolumeWatcher::new(event_tx);
			if let Err(e) = watcher
				.start(device_id.clone(), self_arc_watcher.clone())
				.await
			{
				error!(?e, "Failed to start watcher for volumes");
				return;
			}
		});

		// Start maintenance task
		let self_arc_maintenance = Arc::clone(&self_arc);
		tokio::spawn(async move {
			loop {
				tokio::time::sleep(Duration::from_secs(300)).await;
				let mut actor = self_arc_maintenance.lock().await;
				if let Err(e) = actor.perform_maintenance().await {
					error!(?e, "Volume maintenance error");
				}
			}
		});

		info!("Volume manager actor initialized");
	}

	fn start_event_monitoring(
		self,
		mut event_rx: broadcast::Receiver<VolumeEvent>,
		device_pub_id: DevicePubId,
	) {
		tokio::spawn(async move {
			debug!("Starting volume event monitoring");
			while let Ok(event) = event_rx.recv().await {
				debug!("Volume event received: {:?}", event);

				match event {
					VolumeEvent::VolumeSpeedTested {
						fingerprint,
						read_speed,
						write_speed,
					} => {
						// Get read lock first to check volume existence
						let volume_exists = {
							let state = self.state.read().await;
							state.get_volume(&fingerprint).await.is_some()
						};

						if volume_exists {
							// Then get write lock to update speeds
							let state = self.state.write().await;
							let mut registry = state.registry.write().await;
							if let Some(volume) = registry.get_volume_mut(&fingerprint) {
								volume.read_speed_mbps = Some(read_speed);
								volume.write_speed_mbps = Some(write_speed);
							}
						}
					}
					_ => {
						// Handle other events with a single write lock
						let state = self.state.write().await;
						let mut registry = state.registry.write().await;

						match event {
							VolumeEvent::VolumeAdded(volume) => {
								registry.register_volume(volume);
							}
							VolumeEvent::VolumeRemoved(volume) => {
								let fingerprint = VolumeFingerprint::new(&device_pub_id, &volume);
								registry.remove_volume(&fingerprint);
							}
							VolumeEvent::VolumeUpdated { old: _, new } => {
								registry.update_volume(new);
							}
							VolumeEvent::VolumeMountChanged {
								fingerprint,
								is_mounted,
							} => {
								if let Some(volume) = registry.get_volume_mut(&fingerprint) {
									volume.is_mounted = is_mounted;
								}
							}
							VolumeEvent::VolumeError { fingerprint, error } => {
								if let Some(volume) = registry.get_volume_mut(&fingerprint) {
									volume.error_status = Some(error);
								}
							}
							_ => {}
						}
					}
				}
			}
			warn!("Volume event monitoring ended");
		});
	}

	/// Syncs volume memory state with library database
	pub async fn initialize_for_library(
		&mut self,
		library: Arc<Library>,
	) -> Result<(), VolumeError> {
		use sd_prisma::prisma::device;
		let device_id = DevicePubId::from(self.ctx.device_id.clone());
		let state = self.state.clone();
		let state = state.write().await;
		let mut registry = state.registry.write().await;

		let db_device = library
			.db
			.device()
			.find_unique(device::pub_id::equals(device_id.to_db()))
			.exec()
			.await?
			.ok_or(VolumeError::DeviceNotFound(device_id.to_db()))?;

		// Get volumes from database
		let db_volumes = library
			.db
			.volume()
			.find_many(vec![volume::device_id::equals(Some(db_device.id))])
			.exec()
			.await?
			.into_iter()
			.map(Volume::from)
			.collect::<Vec<_>>();

		let registry_read = state.registry.read().await;
		// Process each volume
		for (fingerprint, volume) in registry_read.volumes() {
			// Find matching database volume
			if let Some(db_volume) = db_volumes
				.iter()
				.find(|db_vol| VolumeFingerprint::new(&device_id, db_vol) == *fingerprint)
			{
				// Update existing volume
				let updated = Volume::merge_with_db(volume, db_volume);
				registry.register_volume(updated.clone());
			} else if volume.mount_type == MountType::System {
				// Create new system volume in database
				let created = volume.create(&library.db, device_id.to_db()).await?;
			}
		}

		Ok(())
	}

	async fn perform_maintenance(&mut self) -> Result<(), VolumeError> {
		let mut state = self.state.write().await;

		Ok(())
	}

	async fn scan_volumes(&mut self) -> Result<(), VolumeError> {
		let mut state = self.state.write().await;
		state.scan_volumes().await
	}

	async fn handle_message(&mut self, msg: VolumeManagerMessage) -> Result<(), VolumeError> {
		trace!("VolumeManagerActor received message: {:?}", msg);
		match msg {
			VolumeManagerMessage::ListSystemVolumes { ack, library } => {
				let result = self.handle_list_system_volumes(library).await;
				let _ = ack.send(result);
			}
			VolumeManagerMessage::ListLibraryVolumes { library, ack } => {
				todo!();
			}
			VolumeManagerMessage::TrackVolume {
				fingerprint,
				library,
				ack,
			} => {
				let result = self.handle_track_volume(library, fingerprint).await;
				let _ = ack.send(result);
			}
			VolumeManagerMessage::UntrackVolume {
				fingerprint,
				library,
				ack,
			} => todo!(),
			VolumeManagerMessage::UpdateVolume { volume, ack } => todo!(),
			VolumeManagerMessage::MountVolume { fingerprint, ack } => todo!(),
			VolumeManagerMessage::UnmountVolume { fingerprint, ack } => {
				let result = self
					.handle_unmount_volume(fingerprint, self.ctx.device_id.clone().into())
					.await;
				let _ = ack.send(result);
			}
			VolumeManagerMessage::SpeedTest {
				fingerprint,
				ack,
				library,
			} => todo!(),
		}
		Ok(())
	}

	/// Lists all volumes currently mounted on the system
	async fn handle_list_system_volumes(
		&self,
		library: Arc<Library>,
	) -> Result<Vec<Volume>, VolumeError> {
		tracing::info!("Listing system volumes for library {}", library.id);

		self.state
			.read()
			.await
			.get_volumes_for_library(library)
			.await
	}

	pub async fn get_volumes(&self) -> Vec<Volume> {
		self.state.read().await.list_volumes().await
	}

	pub async fn volume_exists(&self, fingerprint: VolumeFingerprint) -> bool {
		self.state.read().await.volume_exists(&fingerprint).await
	}

	// async fn handle_list_library_volumes(
	// 	&self,
	// 	library: Arc<Library>,
	// ) -> Result<Vec<Volume>, VolumeError> {
	// 	let device_pub_id = self.ctx.device_id.clone();
	// 	let mut result_volumes = Vec::new();

	// 	// Get currently mounted volumes on this system
	// 	let system_volumes = self.handle_list_system_volumes(library.clone()).await?;

	// 	// Get all volumes from the library database
	// 	let db_volumes = library
	// 		.db
	// 		.volume()
	// 		.find_many(vec![])
	// 		.exec()
	// 		.await?
	// 		.into_iter()
	// 		.map(Volume::from)
	// 		.collect::<Vec<_>>();

	// 	// Create fingerprint maps - create references to avoid moving
	// 	let system_map: HashMap<Vec<u8>, &Volume> = system_volumes
	// 		.iter()
	// 		.map(|v| (v.generate_fingerprint(device_pub_id.clone().into()), v))
	// 		.collect();

	// 	// First add all currently mounted volumes, merged with DB data if available
	// 	for volume in &system_volumes {
	// 		let fingerprint = volume.generate_fingerprint(device_pub_id.clone().into());
	// 		if let Some(db_volume) = db_volumes
	// 			.iter()
	// 			.find(|v| v.generate_fingerprint(device_pub_id.clone().into()) == fingerprint)
	// 		{
	// 			result_volumes.push(Volume::merge_with_db_volume(&volume, db_volume));
	// 		} else {
	// 			result_volumes.push(volume.clone());
	// 		}
	// 	}

	// 	// Then add any database volumes that aren't currently mounted
	// 	for db_volume in db_volumes {
	// 		let fingerprint = db_volume.generate_fingerprint(device_pub_id.clone().into());
	// 		if !system_map.contains_key(&fingerprint) {
	// 			result_volumes.push(db_volume);
	// 		}
	// 	}

	// 	Ok(result_volumes)
	// }

	/// When tracking a volume, we associate it with the current device in the database
	async fn handle_track_volume(
		&mut self,
		library: Arc<Library>,
		fingerprint: VolumeFingerprint,
	) -> Result<(), VolumeError> {
		let state = self.state.write().await;
		let device_pub_id = self.ctx.device_id.clone();

		// Find the volume in our current system volumes
		let mut registry = state.registry.write().await;
		let mut volume = match registry.get_volume_mut(&fingerprint) {
			Some(v) => v.clone(),
			None => return Err(VolumeError::InvalidFingerprint(fingerprint.clone())),
		};

		// Create in database with current device association
		volume.create(&library.db, device_pub_id.into()).await?;

		// Spawn a background task to perform the speed test
		let event_tx = self.event_tx.clone();
		let mut volume = volume.clone();
		tokio::spawn(async move {
			if let Err(e) = volume.speed_test(None, Some(&event_tx)).await {
				error!(?e, "Failed to perform speed test for volume");
			}
		});

		Ok(())
	}

	async fn handle_unmount_volume(
		&mut self,
		fingerprint: VolumeFingerprint,
		device_pub_id: DevicePubId,
	) -> Result<(), VolumeError> {
		let state = self.state.read().await;
		let volume = state
			.get_volume(&fingerprint)
			.await
			.ok_or_else(|| VolumeError::NotFound(fingerprint.clone()))?;

		if !volume.is_mounted {
			return Err(VolumeError::NotMounted(volume.mount_point.clone()));
		}

		// Call platform-specific unmount
		super::os::unmount_volume(&volume.mount_point).await?;

		let fingerprint = VolumeFingerprint::new(&device_pub_id, &volume);

		// Emit unmount event
		if let Some(pub_id) = volume.pub_id.as_ref() {
			let _ = self.event_tx.send(VolumeEvent::VolumeMountChanged {
				fingerprint,
				is_mounted: false,
			});
		}

		Ok(())
	}

	async fn handle_library_deletion(&mut self, library: Arc<Library>) -> Result<(), VolumeError> {
		// Clean up volumes associated with deleted library
		let _state = self.state.write().await;

		// TODO: Implement library deletion cleanup
		// This might involve:
		// 1. Removing volumes only tracked by this library
		// 2. Updating volumes tracked by multiple libraries

		Ok(())
	}
}
