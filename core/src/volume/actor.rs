use super::{
	error::VolumeError,
	types::{Volume, VolumeEvent, VolumeOptions},
	volumes::Volumes,
	watcher::{VolumeWatcher, WatcherState},
	VolumeManagerContext, VolumeManagerState,
};
use crate::{
	library::{Library, LibraryManagerEvent},
	volume::MountType,
};
use async_channel as chan;
use sd_prisma::prisma::{album::pub_id, volume};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::{broadcast, oneshot, Mutex, RwLock};
use tokio::time::Instant;
use tracing::{debug, error, info, trace, warn};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
const DEFAULT_CHANNEL_SIZE: usize = 128;

#[derive(Debug)]
pub enum VolumeManagerMessage {
	TrackVolume {
		volume_id: Vec<u8>,
		library: Arc<Library>,
		ack: oneshot::Sender<Result<(), VolumeError>>,
	},
	UntrackVolume {
		volume_id: Vec<u8>,
		library: Arc<Library>,
		ack: oneshot::Sender<Result<(), VolumeError>>,
	},
	UpdateVolume {
		volume: Volume,
		ack: oneshot::Sender<Result<(), VolumeError>>,
	},
	MountVolume {
		volume_id: Vec<u8>,
		ack: oneshot::Sender<Result<(), VolumeError>>,
	},
	UnmountVolume {
		volume_id: Vec<u8>,
		ack: oneshot::Sender<Result<(), VolumeError>>,
	},
	SpeedTest {
		volume_id: Vec<u8>,
		ack: oneshot::Sender<Result<(), VolumeError>>,
	},
	ListSystemVolumes {
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
		let (event_tx, event_rx) = broadcast::channel(DEFAULT_CHANNEL_SIZE);

		let manager = Volumes::new(message_tx, event_tx.clone());
		let state = VolumeManagerState::new(options, event_tx.clone()).await?;

		let actor = VolumeManagerActor {
			state: Arc::new(RwLock::new(state)),
			message_rx,
			event_tx,
			ctx,
		};

		// Pass event_rx to start monitoring task immediately
		actor
			.clone()
			.start_event_monitoring(event_rx, actor.ctx.device_id.clone());

		Ok((manager, actor))
	}

	fn start_event_monitoring(
		self,
		mut event_rx: broadcast::Receiver<VolumeEvent>,
		current_device_pub_id: Vec<u8>,
	) {
		tokio::spawn(async move {
			debug!("Starting volume event monitoring");
			while let Ok(event) = event_rx.recv().await {
				debug!("Volume event received: {:?}", event);

				match event {
					VolumeEvent::VolumeAdded(volume) => {
						self.state.write().await.volumes.insert(
							volume.generate_fingerprint(current_device_pub_id.clone()),
							volume,
						);
					}
					VolumeEvent::VolumeRemoved(volume) => {
						self.state
							.write()
							.await
							.volumes
							.remove(&volume.generate_fingerprint(current_device_pub_id.clone()));
					}
					VolumeEvent::VolumeUpdated { old, new } => todo!(),
					VolumeEvent::VolumeSpeedTested {
						id,
						read_speed,
						write_speed,
					} => {
						self.state
							.write()
							.await
							.volumes
							.get_mut(&id)
							.unwrap()
							.read_speed_mbps = Some(read_speed);
						self.state
							.write()
							.await
							.volumes
							.get_mut(&id)
							.unwrap()
							.write_speed_mbps = Some(write_speed);
					}
					VolumeEvent::VolumeMountChanged { id, is_mounted } => todo!(),
					VolumeEvent::VolumeError { id, error } => todo!(),
				}
			}
			warn!("Volume event monitoring ended");
		});
	}

	/// Starts the VolumeManagerActor
	pub async fn start(self, device_pub_id: Vec<u8>) {
		info!("Volume manager actor started");
		let self_arc = Arc::new(Mutex::new(self));

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

		// Subscribe to LibraryManagerEvent
		let self_arc_subscribe = Arc::clone(&self_arc);
		let rx = {
			let actor = self_arc.lock().await;
			actor.ctx.library_event_tx.clone()
		};

		// Scan for volumes on startup
		let _ = self_arc.lock().await.scan_volumes().await;

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
									// TODO: check if active library somehow, as we don't care to sync volumes for inactive libraries
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

		tokio::spawn(async move {
			// start one watcher
			let watcher = VolumeWatcher::new(event_tx);
			if let Err(e) = watcher.start(device_pub_id.clone(), self_arc.clone()).await {
				error!(?e, "Failed to start watcher for volumes");
				return;
			}
		});

		info!("Volume manager actor initialized");
	}

	/// Syncs volume memory state with library database
	pub async fn initialize_for_library(
		&mut self,
		library: Arc<Library>,
	) -> Result<(), VolumeError> {
		use sd_prisma::prisma::device;
		// Use device_id from context instead of node
		let device_pub_id = self.ctx.device_id.clone();
		let current_volumes = self.get_volumes().await;

		let db_device = library
			.db
			.device()
			.find_unique(device::pub_id::equals(device_pub_id.clone()))
			.exec()
			.await?
			.ok_or(VolumeError::DeviceNotFound(device_pub_id.clone()))?;

		let db_system_volumes = library
			.db
			.volume()
			.find_many(vec![
				volume::device_id::equals(Some(db_device.id)),
				volume::mount_type::equals(Some(MountType::System.to_string())),
			])
			.exec()
			.await?;

		let db_system_volumes = db_system_volumes.into_iter().map(Volume::from);

		// Register system volumes in the db
		if db_system_volumes.len() == 0 {
			for v in current_volumes.iter() {
				if v.mount_type == MountType::System {
					// Create is will always treat the volume as a new volume
					// Assigning a new pub_id in the process
					v.create(&library.db, device_pub_id.clone()).await?;
				}
			}
		}

		info!(
			"Volume manager initialized for library: {:?}",
			self.state.read().await.volumes
		);

		Ok(())
	}

	async fn perform_maintenance(&mut self) -> Result<(), VolumeError> {
		let mut state = self.state.write().await;

		// Pass device_id to maintenance
		if let Err(e) = state.maintenance(self.ctx.device_id.clone()).await {
			error!(?e, "Volume maintenance error");
		}

		// Rescan volumes periodically
		if state.last_scan.elapsed() > Duration::from_secs(300) {
			drop(state);
			self.scan_volumes().await?;
			state = self.state.write().await;
		}

		Ok(())
	}

	async fn scan_volumes(&mut self) -> Result<(), VolumeError> {
		let mut state = self.state.write().await;
		state.scan_volumes(self.ctx.device_id.clone()).await
	}

	async fn handle_message(&mut self, msg: VolumeManagerMessage) -> Result<(), VolumeError> {
		trace!("VolumeManagerActor received message: {:?}", msg);
		match msg {
			VolumeManagerMessage::ListSystemVolumes { ack } => {
				let result = self.handle_list_system_volumes().await;
				let _ = ack.send(result);
			}
			VolumeManagerMessage::ListLibraryVolumes { library, ack } => {
				let result = self.handle_list_library_volumes(library).await;
				let _ = ack.send(result);
			}
			VolumeManagerMessage::TrackVolume {
				volume_id,
				library,
				ack,
			} => {
				let result = self.handle_track_volume(library, volume_id).await;
				let _ = ack.send(result);
			}
			VolumeManagerMessage::UntrackVolume {
				volume_id,
				library,
				ack,
			} => todo!(),
			VolumeManagerMessage::UpdateVolume { volume, ack } => todo!(),
			VolumeManagerMessage::MountVolume { volume_id, ack } => todo!(),
			VolumeManagerMessage::UnmountVolume { volume_id, ack } => todo!(),
			VolumeManagerMessage::SpeedTest { volume_id, ack } => todo!(),
		}
		Ok(())
	}
	/// Lists all volumes currently mounted on the system
	async fn handle_list_system_volumes(&self) -> Result<Vec<Volume>, VolumeError> {
		tracing::info!(
			"Currently {} volumes present in the system",
			self.state.read().await.volumes.len()
		);
		// Return volumes from state instead of rescanning
		Ok(self.state.read().await.volumes.values().cloned().collect())
	}

	pub async fn get_volumes(&self) -> Vec<Volume> {
		self.state.read().await.volumes.values().cloned().collect()
	}

	pub async fn volume_exists(&self, fingerprint: Vec<u8>) -> bool {
		self.state.read().await.volumes.contains_key(&fingerprint)
	}

	async fn handle_list_library_volumes(
		&self,
		library: Arc<Library>,
	) -> Result<Vec<Volume>, VolumeError> {
		let device_pub_id = self.ctx.device_id.clone();
		let mut result_volumes = Vec::new();

		// Get currently mounted volumes on this system
		let system_volumes = self.handle_list_system_volumes().await?;

		// Get all volumes from the library database
		let db_volumes = library
			.db
			.volume()
			.find_many(vec![])
			.exec()
			.await?
			.into_iter()
			.map(Volume::from)
			.collect::<Vec<_>>();

		// Create fingerprint maps - create references to avoid moving
		let system_map: HashMap<Vec<u8>, &Volume> = system_volumes
			.iter()
			.map(|v| (v.generate_fingerprint(device_pub_id.clone().into()), v))
			.collect();

		// First add all currently mounted volumes, merged with DB data if available
		for volume in &system_volumes {
			let fingerprint = volume.generate_fingerprint(device_pub_id.clone().into());
			if let Some(db_volume) = db_volumes
				.iter()
				.find(|v| v.generate_fingerprint(device_pub_id.clone().into()) == fingerprint)
			{
				result_volumes.push(Volume::merge_with_db_volume(&volume, db_volume));
			} else {
				result_volumes.push(volume.clone());
			}
		}

		// Then add any database volumes that aren't currently mounted
		for db_volume in db_volumes {
			let fingerprint = db_volume.generate_fingerprint(device_pub_id.clone().into());
			if !system_map.contains_key(&fingerprint) {
				result_volumes.push(db_volume);
			}
		}

		Ok(result_volumes)
	}

	/// When tracking a volume, we associate it with the current device in the database
	async fn handle_track_volume(
		&mut self,
		library: Arc<Library>,
		volume_id: Vec<u8>,
	) -> Result<(), VolumeError> {
		let state = self.state.write().await;
		let device_pub_id = self.ctx.device_id.clone();

		// Find the volume in our current system volumes
		let volume = match state.volumes.get(&volume_id) {
			Some(v) => v.clone(),
			None => return Err(VolumeError::InvalidId(hex::encode(&volume_id))),
		};

		// Create in database with current device association
		volume.create(&library.db, device_pub_id.into()).await?;

		Ok(())
	}

	async fn handle_library_deletion(&mut self, library: Arc<Library>) -> Result<(), VolumeError> {
		// Clean up volumes associated with deleted library
		let _state = self.state.write().await;

		// TODO: Implement library deletion cleanup
		// This might involve:
		// 1. Removing volumes only tracked by this library
		// 2. Updating volumes tracked by multiple libraries
		// 3. Cleaning up watchers

		Ok(())
	}
}
