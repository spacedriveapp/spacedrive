use std::{
	borrow::Cow,
	collections::HashMap,
	path::PathBuf,
	sync::Arc,
	time::{Duration, Instant},
};

use futures::Stream;
use sd_p2p::{
	spaceblock::{BlockSize, SpaceblockRequest, Transfer},
	spacetunnel::{RemoteIdentity, Tunnel},
	Event, Manager, ManagerError, ManagerStream, MetadataManager, PeerId,
};
use serde::Serialize;
use specta::Type;
use tokio::{
	fs::File,
	io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader},
	sync::{broadcast, oneshot, Mutex},
	time::sleep,
};
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::{
	library::LibraryManager,
	node::{NodeConfig, NodeConfigManager},
	p2p::{OperatingSystem, SPACEDRIVE_APP_ID},
};

use super::{
	sync::{NetworkedLibraryManager, SyncMessage},
	Header, PairingManager, PairingStatus, PeerMetadata,
};

/// The amount of time to wait for a Spacedrop request to be accepted or rejected before it's automatically rejected
const SPACEDROP_TIMEOUT: Duration = Duration::from_secs(60);

/// TODO: P2P event for the frontend
#[derive(Debug, Clone, Type, Serialize)]
#[serde(tag = "type")]
pub enum P2PEvent {
	DiscoveredPeer {
		peer_id: PeerId,
		metadata: PeerMetadata,
	},
	SpacedropRequest {
		id: Uuid,
		peer_id: PeerId,
		name: String,
	},
	// Pairing was reuqest has come in.
	// This will fire on the responder only.
	PairingRequest {
		id: u16,
		name: String,
		os: OperatingSystem,
	},
	PairingProgress {
		id: u16,
		status: PairingStatus,
	}, // TODO: Expire peer + connection/disconnect
}

pub struct P2PManager {
	pub events: (broadcast::Sender<P2PEvent>, broadcast::Receiver<P2PEvent>),
	pub manager: Arc<Manager<PeerMetadata>>,
	spacedrop_pairing_reqs: Arc<Mutex<HashMap<Uuid, oneshot::Sender<Option<String>>>>>,
	pub metadata_manager: Arc<MetadataManager<PeerMetadata>>,
	pub spacedrop_progress: Arc<Mutex<HashMap<Uuid, broadcast::Sender<u8>>>>,
	pub pairing: Arc<PairingManager>,
	node_config_manager: Arc<NodeConfigManager>,
}

impl P2PManager {
	pub async fn new(
		node_config: Arc<NodeConfigManager>,
	) -> Result<(Arc<P2PManager>, ManagerStream<PeerMetadata>), ManagerError> {
		let (config, keypair) = {
			let config = node_config.get().await;

			// TODO: The `vec![]` here is problematic but will be fixed with delayed `MetadataManager`
			(Self::config_to_metadata(&config, vec![]), config.keypair)
		};

		// TODO: Delay building this until the libraries are loaded
		let metadata_manager = MetadataManager::new(config);

		let (manager, stream) =
			Manager::new(SPACEDRIVE_APP_ID, &keypair, metadata_manager.clone()).await?;

		info!(
			"Node '{}' is now online listening at addresses: {:?}",
			manager.peer_id(),
			manager.listen_addrs().await
		);

		// need to keep 'rx' around so that the channel isn't dropped
		let (tx, rx) = broadcast::channel(100);

		let spacedrop_pairing_reqs = Arc::new(Mutex::new(HashMap::new()));
		let spacedrop_progress = Arc::new(Mutex::new(HashMap::new()));

		let pairing = PairingManager::new(manager.clone(), tx.clone(), metadata_manager.clone());

		// TODO: proper shutdown
		// https://docs.rs/ctrlc/latest/ctrlc/
		// https://docs.rs/system_shutdown/latest/system_shutdown/

		Ok((
			Arc::new(Self {
				pairing,
				events: (tx, rx),
				manager,
				spacedrop_pairing_reqs,
				metadata_manager,
				spacedrop_progress,
				node_config_manager: node_config,
			}),
			stream,
		))
	}

	pub fn start(
		&self,
		mut stream: ManagerStream<PeerMetadata>,
		library_manager: Arc<LibraryManager>,
		nlm: Arc<NetworkedLibraryManager>,
	) {
		tokio::spawn({
			let manager = self.manager.clone();
			let metadata_manager = self.metadata_manager.clone();
			let events = self.events.0.clone();
			let spacedrop_pairing_reqs = self.spacedrop_pairing_reqs.clone();
			let spacedrop_progress = self.spacedrop_progress.clone();
			let pairing = self.pairing.clone();

			async move {
				let mut shutdown = false;
				while let Some(event) = stream.next().await {
					match event {
						Event::PeerDiscovered(event) => {
							debug!(
								"Discovered peer by id '{}' with address '{:?}' and metadata: {:?}",
								event.peer_id, event.addresses, event.metadata
							);

							events
								.send(P2PEvent::DiscoveredPeer {
									peer_id: event.peer_id,
									metadata: event.metadata.clone(),
								})
								.map_err(|_| error!("Failed to send event to p2p event stream!"))
								.ok();

							nlm.peer_discovered(event).await;
						}
						Event::PeerExpired { id, metadata } => {
							debug!("Peer '{}' expired with metadata: {:?}", id, metadata);
							nlm.peer_expired(id).await;
						}
						Event::PeerConnected(event) => {
							debug!("Peer '{}' connected", event.peer_id);
							nlm.peer_connected(event.peer_id).await;

							if event.establisher {
								let manager = manager.clone();
								let nlm = nlm.clone();
								let instances = metadata_manager.get().instances;
								tokio::spawn(async move {
									let mut stream = manager.stream(event.peer_id).await.unwrap();
									Self::resync(nlm, &mut stream, event.peer_id, instances).await;
								});
							}
						}
						Event::PeerDisconnected(peer_id) => {
							debug!("Peer '{}' disconnected", peer_id);
							nlm.peer_disconnected(peer_id).await;
						}
						Event::PeerMessage(event) => {
							let events = events.clone();
							let metadata_manager = metadata_manager.clone();
							let spacedrop_pairing_reqs = spacedrop_pairing_reqs.clone();
							let spacedrop_progress = spacedrop_progress.clone();
							let pairing = pairing.clone();

							let library_manager = library_manager.clone();
							let nlm = nlm.clone();

							tokio::spawn(async move {
								let mut stream = event.stream;
								let header = Header::from_stream(&mut stream).await.unwrap();

								match header {
									Header::Ping => {
										debug!("Received ping from peer '{}'", event.peer_id);
									}
									Header::Spacedrop(req) => {
										let id = Uuid::new_v4();
										let (tx, rx) = oneshot::channel();

										info!("spacedrop({id}): received from peer '{}' for file '{}' with file length '{}'", event.peer_id, req.name, req.size);

										spacedrop_pairing_reqs.lock().await.insert(id, tx);

										let (process_tx, _) = broadcast::channel(100);
										spacedrop_progress
											.lock()
											.await
											.insert(id, process_tx.clone());

										if events
											.send(P2PEvent::SpacedropRequest {
												id,
												peer_id: event.peer_id,
												name: req.name.clone(),
											})
											.is_err()
										{
											// No frontend's are active

											todo!("Outright reject Spacedrop");
										}

										tokio::select! {
											_ = sleep(SPACEDROP_TIMEOUT) => {
												info!("spacedrop({id}): timeout, rejecting!");
											}
											file_path = rx => {
												match file_path {
													Ok(Some(file_path)) => {
														info!("spacedrop({id}): accepted saving to '{:?}'", file_path);

														stream.write_all(&[1]).await.unwrap();

														let f = File::create(file_path).await.unwrap();

														Transfer::new(&req, |percent| {
															process_tx.send(percent).ok();
														}).receive(&mut stream, f).await;

														info!("spacedrop({id}): complete");
													}
													Ok(None) => {
														info!("spacedrop({id}): rejected");
													}
													Err(_) => {
														info!("spacedrop({id}): error with Spacedrop pairing request receiver!");
													}
												}
											}
										};
									}
									Header::Pair => {
										pairing
											.responder(event.peer_id, stream, &library_manager)
											.await;
									}
									Header::Sync(library_id) => {
										// Header -> Tunnel -> SyncMessage

										let mut tunnel = Tunnel::responder(stream).await.unwrap();

										let msg =
											SyncMessage::from_stream(&mut tunnel).await.unwrap();

										let library =
											library_manager.get_library(&library_id).await.unwrap();

										dbg!(&msg);

										let ingest = &library.sync.ingest;

										match msg {
											SyncMessage::NewOperations => {
												// The ends up in `NetworkedLibraryManager::request_and_ingest_ops`.
												// TODO: Throw tunnel around like this makes it soooo confusing.
												ingest.notify(tunnel, event.peer_id).await;
											}
											SyncMessage::OperationsRequest(_) => {
												nlm.exchange_sync_ops(
													tunnel,
													&event.peer_id,
													library_id,
													&library.sync,
												)
												.await;
											}
											SyncMessage::OperationsRequestResponse(_) => {
												todo!("unreachable but add proper error handling")
											}
										};
									}
									Header::Connected(identities) => {
										Self::resync_handler(
											nlm,
											&mut stream,
											event.peer_id,
											metadata_manager.get().instances,
											identities,
										)
										.await
									}
								}
							});
						}
						Event::PeerBroadcast(_event) => {
							// todo!();
						}
						Event::Shutdown => {
							shutdown = true;
							break;
						}
						_ => debug!("event: {:?}", event),
					}
				}

				if !shutdown {
					error!(
						"Manager event stream closed! The core is unstable from this point forward!"
					);
				}
			}
		});
	}

	fn config_to_metadata(config: &NodeConfig, instances: Vec<RemoteIdentity>) -> PeerMetadata {
		PeerMetadata {
			name: config.name.clone(),
			operating_system: Some(OperatingSystem::get_os()),
			version: Some(env!("CARGO_PKG_VERSION").to_string()),
			email: config.p2p_email.clone(),
			img_url: config.p2p_img_url.clone(),
			instances,
		}
	}

	// TODO: Remove this & move to `NetworkedLibraryManager`??? or make it private?
	pub async fn update_metadata(&self, instances: Vec<RemoteIdentity>) {
		self.metadata_manager.update(Self::config_to_metadata(
			&self.node_config_manager.get().await,
			instances,
		));
	}

	pub async fn resync(
		nlm: Arc<NetworkedLibraryManager>,
		stream: &mut (impl AsyncRead + AsyncWrite + Unpin),
		peer_id: PeerId,
		instances: Vec<RemoteIdentity>,
	) {
		// TODO: Make this encrypted using node to node auth so it can't be messed with in transport

		stream
			.write_all(&Header::Connected(instances).to_bytes())
			.await
			.unwrap();

		let Header::Connected(identities) =
			Header::from_stream(stream).await.unwrap() else {
				panic!("unreachable but error handling")
			};

		for identity in identities {
			nlm.peer_connected2(identity, peer_id).await;
		}
	}

	pub async fn resync_handler(
		nlm: Arc<NetworkedLibraryManager>,
		stream: &mut (impl AsyncRead + AsyncWrite + Unpin),
		peer_id: PeerId,
		local_identities: Vec<RemoteIdentity>,
		remote_identities: Vec<RemoteIdentity>,
	) {
		for identity in remote_identities {
			nlm.peer_connected2(identity, peer_id).await;
		}

		stream
			.write_all(&Header::Connected(local_identities).to_bytes())
			.await
			.unwrap();
	}

	pub async fn accept_spacedrop(&self, id: Uuid, path: String) {
		if let Some(chan) = self.spacedrop_pairing_reqs.lock().await.remove(&id) {
			chan.send(Some(path)).unwrap();
		}
	}

	pub async fn reject_spacedrop(&self, id: Uuid) {
		if let Some(chan) = self.spacedrop_pairing_reqs.lock().await.remove(&id) {
			chan.send(None).unwrap();
		}
	}

	pub fn subscribe(&self) -> broadcast::Receiver<P2PEvent> {
		self.events.0.subscribe()
	}

	pub async fn ping(&self) {
		self.manager.broadcast(Header::Ping.to_bytes()).await;
	}

	// TODO: Proper error handling
	pub async fn big_bad_spacedrop(
		&self,
		peer_id: PeerId,
		path: PathBuf,
	) -> Result<Option<Uuid>, ()> {
		let id = Uuid::new_v4();
		let (tx, _) = broadcast::channel(25);
		let mut stream = self.manager.stream(peer_id).await.map_err(|_| ())?; // TODO: handle providing incorrect peer id

		let file = File::open(&path).await.map_err(|_| ())?;
		let metadata = file.metadata().await.map_err(|_| ())?;

		let header = Header::Spacedrop(SpaceblockRequest {
			name: path
				.file_name()
				.map(|v| v.to_string_lossy())
				.unwrap_or(Cow::Borrowed(""))
				.to_string(),
			size: metadata.len(),
			block_size: BlockSize::from_size(metadata.len()), // TODO: This should be dynamic
		});
		stream.write_all(&header.to_bytes()).await.map_err(|_| ())?;

		debug!("Waiting for Spacedrop to be accepted from peer '{peer_id}'");
		let mut buf = [0; 1];
		// TODO: Add timeout so the connection is dropped if they never response
		stream.read_exact(&mut buf).await.map_err(|_| ())?;
		if buf[0] != 1 {
			debug!("Spacedrop was rejected from peer '{peer_id}'");
			return Ok(None);
		}

		debug!("Starting Spacedrop to peer '{peer_id}'");
		let i = Instant::now();

		let file = BufReader::new(file);
		self.spacedrop_progress.lock().await.insert(id, tx.clone());
		Transfer::new(
			&match header {
				Header::Spacedrop(req) => req,
				_ => unreachable!(),
			},
			|percent| {
				tx.send(percent).ok();
			},
		)
		.send(&mut stream, file)
		.await;

		debug!(
			"Finished Spacedrop to peer '{peer_id}' after '{:?}",
			i.elapsed()
		);

		Ok(Some(id))
	}

	pub async fn spacedrop_progress(&self, id: Uuid) -> Option<impl Stream<Item = u8>> {
		self.spacedrop_progress.lock().await.get(&id).map(|v| {
			let mut v = v.subscribe();
			async_stream::stream! {
				while let Ok(item) = v.recv().await {
					yield item;
				}
			}
		})
	}

	pub async fn shutdown(&self) {
		self.manager.shutdown().await;
	}
}
