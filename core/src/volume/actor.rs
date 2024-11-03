use super::{
	error::VolumeError,
	types::{Volume, VolumeEvent, VolumeOptions},
	volumes::Volumes,
	watcher::{VolumeWatcher, WatcherState},
	VolumeManagerContext, VolumeManagerState,
};
use crate::library::{Library, LibraryManagerEvent};
use async_channel as chan;
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
	WatchVolume {
		volume_id: Vec<u8>,
		ack: oneshot::Sender<Result<(), VolumeError>>,
	},
	UnwatchVolume {
		volume_id: Vec<u8>,
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
		actor.clone().start_event_monitoring(event_rx);

		Ok((manager, actor))
	}

	fn start_event_monitoring(self, mut event_rx: broadcast::Receiver<VolumeEvent>) {
		tokio::spawn(async move {
			debug!("Starting volume event monitoring");
			while let Ok(event) = event_rx.recv().await {
				debug!("Volume event processed: {:?}", event);
			}
			warn!("Volume event monitoring ended");
		});
	}

	/// Starts the VolumeManagerActor
	pub async fn start(self) {
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
									actor.initialize(library.clone()).await
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

		// Start the volume watcher
		let self_arc_watcher = Arc::clone(&self_arc);
		tokio::spawn(async move {
			let mut actor = self_arc_watcher.lock().await;
			let state = actor.state.write().await;

			// Create and start watcher for each volume
			for (volume_id, volume) in &state.volumes {
				if let Some(watcher) = state.watchers.get(volume_id) {
					if let Err(e) = watcher.watcher.start().await {
						error!(?e, "Failed to start watcher for volume {}", volume.name);
					}
				}
			}
		});

		info!("Volume manager actor initialized");
	}

	pub async fn initialize(&mut self, library: Arc<Library>) -> Result<(), VolumeError> {
		// Use device_id from context instead of node
		let device_pub_id = self.ctx.device_id.clone();

		// Scan for system volumes first
		{
			let mut state = self.state.write().await;
			state.scan_volumes(device_pub_id.clone()).await?;
		}

		// Get volumes from library database
		let db_volumes = library
			.db
			.volume()
			.find_many(vec![])
			.exec()
			.await?
			.into_iter()
			.map(Volume::from);

		// Get current volumes and clone what we need
		let current_volumes = {
			let state = self.state.read().await;
			state.volumes.clone()
		};

		let mut updates = Vec::new();

		// Prepare updates
		for db_volume in db_volumes {
			let fingerprint = db_volume.generate_fingerprint(device_pub_id.clone().into());

			if let Some(system_volume) = current_volumes
				.values()
				.find(|v| v.generate_fingerprint(device_pub_id.clone().into()) == fingerprint)
			{
				let merged = Volume::merge_with_db_volume(system_volume, &db_volume);
				if let Some(pub_id) = &merged.pub_id {
					updates.push((pub_id.clone(), merged.clone()));
					let _ = self.event_tx.send(VolumeEvent::VolumeUpdated {
						old: system_volume.clone(),
						new: merged,
					});
				}
			} else if let Some(pub_id) = &db_volume.pub_id {
				updates.push((pub_id.clone(), db_volume.clone()));
				let _ = self.event_tx.send(VolumeEvent::VolumeAdded(db_volume));
			}
		}

		// Apply updates and initialize watchers
		{
			let mut state = self.state.write().await;

			// Update volumes
			for (pub_id, volume) in updates {
				state.volumes.insert(pub_id.clone(), volume);

				// Create and start watcher if it doesn't exist
				if !state.watchers.contains_key(&pub_id) {
					let watcher = VolumeWatcher::new(self.event_tx.clone());
					if let Err(e) = watcher.start().await {
						error!(
							?e,
							"Failed to start watcher for volume {}",
							hex::encode(&pub_id)
						);
						continue;
					}

					state.watchers.insert(
						pub_id,
						WatcherState {
							watcher: Arc::new(watcher),
							last_event: Instant::now(),
							paused: false,
						},
					);
				}
			}

			// Remove any watchers for volumes that no longer exist
			let stale_watchers: Vec<_> = state
				.watchers
				.keys()
				.filter(|id| !state.volumes.contains_key(*id))
				.cloned()
				.collect();

			for volume_id in stale_watchers {
				if let Some(watcher_state) = state.watchers.remove(&volume_id) {
					watcher_state.watcher.stop().await;
				}
			}
		}

		info!(
			"Volume manager initialized: {:?}",
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

		// Clean up stale watchers
		let stale_watchers: Vec<_> = state
			.watchers
			.iter()
			.filter(|(_, state)| state.last_event.elapsed() > Duration::from_secs(3600))
			.map(|(id, _)| id.clone())
			.collect();

		for volume_id in stale_watchers {
			if let Some(watcher_state) = state.watchers.get(&volume_id) {
				watcher_state.watcher.stop().await;
			}
			state.watchers.remove(&volume_id);
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
				tracing::info!("Handling ListSystemVolumes request");
				let result = self.handle_list_system_volumes().await;
				if let Ok(volumes) = &result {
					tracing::info!("Found {} volumes to return", volumes.len());
				}
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
			VolumeManagerMessage::WatchVolume { volume_id, ack } => todo!(),
			VolumeManagerMessage::UnwatchVolume { volume_id, ack } => todo!(),
			VolumeManagerMessage::MountVolume { volume_id, ack } => todo!(),
			VolumeManagerMessage::UnmountVolume { volume_id, ack } => todo!(),
			VolumeManagerMessage::SpeedTest { volume_id, ack } => todo!(),
		}
		Ok(())
	}
	/// Lists all volumes currently mounted on the system
	async fn handle_list_system_volumes(&self) -> Result<Vec<Volume>, VolumeError> {
		tracing::info!(
			"Found {} system volumes",
			self.state.read().await.volumes.len()
		);
		// Return volumes from state instead of rescanning
		Ok(self.state.read().await.volumes.values().cloned().collect())
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
