use std::{
	borrow::Cow,
	collections::HashMap,
	path::PathBuf,
	sync::Arc,
	time::{Duration, Instant},
};

use futures::Stream;
use sd_core_sync::{ingest, SyncManager};
use sd_p2p::{
	spaceblock::{BlockSize, SpaceblockRequest, Transfer},
	spacetunnel::{Identity, Tunnel},
	Event, Manager, ManagerError, ManagerStream, MetadataManager, PeerId,
};
use sd_sync::CRDTOperation;
use serde::Serialize;
use specta::Type;
use tokio::{
	fs::File,
	io::{AsyncReadExt, AsyncWriteExt, BufReader},
	sync::{broadcast, oneshot, Mutex, RwLock},
	time::sleep,
};
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::{
	library::LibraryManager,
	node::{NodeConfig, NodeConfigManager},
	p2p::{OperatingSystem, SPACEDRIVE_APP_ID},
};

use super::{Header, PairingManager, PairingStatus, PeerMetadata};

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
	instances: RwLock<Vec<Arc<Identity>>>,
	node_config_manager: Arc<NodeConfigManager>,
}

impl P2PManager {
	pub async fn new(
		node_config: Arc<NodeConfigManager>,
	) -> Result<(Arc<P2PManager>, ManagerStream<PeerMetadata>), ManagerError> {
		let (config, keypair) = {
			let config = node_config.get().await;
			(Self::config_to_metadata(&config), config.keypair)
		};

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

		let pairing = PairingManager::new(manager.clone(), tx.clone());

		// TODO: proper shutdown
		// https://docs.rs/ctrlc/latest/ctrlc/
		// https://docs.rs/system_shutdown/latest/system_shutdown/

		let this = Arc::new(Self {
			pairing,
			events: (tx, rx),
			manager,
			spacedrop_pairing_reqs,
			metadata_manager,
			spacedrop_progress,
			instances: Default::default(),
			node_config_manager: node_config,
		});

		// TODO: Probs remove this once connection timeout/keepalive are working correctly
		tokio::spawn({
			let this = this.clone();
			async move {
				loop {
					tokio::time::sleep(std::time::Duration::from_secs(5)).await;
					this.ping().await;
				}
			}
		});

		Ok((this, stream))
	}

	pub fn start(
		&self,
		mut stream: ManagerStream<PeerMetadata>,
		library_manager: Arc<LibraryManager>,
	) {
		tokio::spawn({
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

							// TODO: Don't just connect to everyone when we find them. We should only do it if we know them.
							// TODO(Spacedrop): Disable Spacedrop for now
							// event.dial().await;
						}
						Event::PeerMessage(event) => {
							let events = events.clone();
							let spacedrop_pairing_reqs = spacedrop_pairing_reqs.clone();
							let spacedrop_progress = spacedrop_progress.clone();
							let library_manager = library_manager.clone();
							let pairing = pairing.clone();

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
											.responder(event.peer_id, stream, library_manager)
											.await;
									}
									Header::Sync(library_id) => {
										let mut tunnel = Tunnel::responder(stream).await.unwrap();

										let msg =
											SyncMessage::from_tunnel(&mut tunnel).await.unwrap();

										let library =
											library_manager.get_library(library_id).await.unwrap();

										dbg!(&msg);

										let ingest = &library.sync.ingest;

										match msg {
											SyncMessage::NewOperations => {
												ingest.notify(event.peer_id).await;
											}
											SyncMessage::OperationsRequest(v) => {
												tunnel
													.write_all(
														&SyncMessage::OperationsRequestResponse(v)
															.to_bytes(library_id),
													)
													.await
													.unwrap();
											}
											SyncMessage::OperationsRequestResponse(v) => {
												// ingest
												// 	.events
												// 	.send(ingest::Event::Messages(v))
												// 	.await
												// 	.ok();
											}
										};

										tunnel.flush().await.unwrap();
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

	fn config_to_metadata(config: &NodeConfig) -> PeerMetadata {
		PeerMetadata {
			name: config.name.clone(),
			operating_system: Some(OperatingSystem::get_os()),
			version: Some(env!("CARGO_PKG_VERSION").to_string()),
			email: config.p2p_email.clone(),
			img_url: config.p2p_img_url.clone(),
			instances: vec![],
		}
	}

	pub async fn add_instance(&self, instance: &Arc<Identity>) {
		let mut instances = self.instances.write().await;

		if !instances.iter().any(|i| Arc::ptr_eq(i, instance)) {
			instances.push(instance.clone());
		}

		self.update_metadata().await;
	}

	pub async fn remove_instance(&self, instance: &Arc<Identity>) {
		self.instances
			.write()
			.await
			.retain(|i| !Arc::ptr_eq(i, instance));

		self.update_metadata().await;
	}

	#[allow(unused)] // TODO: Should probs be using this
	async fn update_metadata(&self) {
		self.metadata_manager.update(PeerMetadata {
			instances: self
				.instances
				.read()
				.await
				.iter()
				.map(|i| hex::encode(i.public_key().to_bytes()))
				.collect(),
			..Self::config_to_metadata(&self.node_config_manager.get().await)
		});
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

	pub async fn broadcast_sync_events(
		&self,
		library_id: Uuid,
		_identity: &Identity,
		event: Vec<CRDTOperation>,
		library_manager: &LibraryManager,
	) {
		println!("broadcasting sync events!");

		let mut buf = match rmp_serde::to_vec_named(&event) {
			Ok(buf) => buf,
			Err(e) => {
				error!("Failed to serialize sync event: {:?}", e);
				return;
			}
		};
		let mut head_buf = Header::Sync(library_id).to_bytes(); // Max Sync payload is like 4GB
		head_buf.extend_from_slice(&(buf.len() as u32).to_le_bytes());
		head_buf.append(&mut buf);

		// TODO: Determine which clients we share that library with

		// TODO: Establish a connection to them

		let _library = library_manager.get_library(library_id).await.unwrap();

		todo!();

		// TODO: probs cache this query in memory cause this is gonna be stupid frequent
		// let target_nodes = library
		// 	.db
		// 	.node()
		// 	.find_many(vec![])
		// 	.exec()
		// 	.await
		// 	.unwrap()
		// 	.into_iter()
		// 	.map(|n| {
		// 		PeerId::from_str(&n.node_peer_id.expect("Node was missing 'node_peer_id'!"))
		// 			.unwrap()
		// 	})
		// 	.collect::<Vec<_>>();

		// info!(
		// 	"Sending sync messages for library '{}' to nodes with peer id's '{:?}'",
		// 	library_id, target_nodes
		// );

		// // TODO: Do in parallel
		// for peer_id in target_nodes {
		// 	let stream = self.manager.stream(peer_id).await.map_err(|_| ()).unwrap(); // TODO: handle providing incorrect peer id

		// 	let mut tunnel = Tunnel::from_stream(stream).await.unwrap();

		// 	tunnel.write_all(&head_buf).await.unwrap();
		// }
	}

	pub async fn alert_new_sync_events(&self, library_id: Uuid, library_manager: &LibraryManager) {
		// let library = library_manager.get_library(library_id).await.unwrap();

		let peers = self.manager.get_connected_peers().await.unwrap();

		// let instances = self.instances.read().await;

		// let target_nodes = library
		// 	.db
		// 	.instance()
		// 	.find_many(vec![])
		// 	.exec()
		// 	.await
		// 	.unwrap()
		// 	.into_iter()
		// 	.map(|n| {
		// 		PeerId::from_str(&n.node_peer_id.expect("Node was missing 'node_peer_id'!"))
		// 			.unwrap()
		// 	})
		// 	.collect::<Vec<_>>();

		// // TODO: Do in parallel
		for peer_id in peers {
			let stream = self.manager.stream(peer_id).await.map_err(|_| ()).unwrap(); // TODO: handle providing incorrect peer id

			let mut tunnel = Tunnel::initiator(stream).await.unwrap();

			tunnel
				.write_all(SyncMessage::NewOperations.to_bytes(library_id).as_slice())
				.await
				.unwrap();
		}
	}

	// TODO: Don't take `PeerId` as an argument
	pub async fn emit_sync_ingest_alert(
		&self,
		sync: &Arc<SyncManager>,
		library_id: Uuid,
		peer_id: PeerId,
		v: u8,
	) {
		let stream = self.manager.stream(peer_id).await.unwrap();

		let mut tunnel = Tunnel::initiator(stream).await.unwrap();

		tunnel
			.write_all(&SyncMessage::OperationsRequest(v).to_bytes(library_id))
			.await
			.unwrap();
		tunnel.flush().await.unwrap();

		let msg = SyncMessage::from_tunnel(&mut tunnel).await.unwrap();

		match msg {
			SyncMessage::OperationsRequestResponse(byte) => {
				sync.ingest
					.events
					.send(ingest::Event::Messages(byte))
					.await
					.ok();
			}
			_ => {}
		};
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

#[derive(Debug)]
#[repr(u8)]
pub enum SyncMessage {
	NewOperations,
	OperationsRequest(u8),
	OperationsRequestResponse(u8),
}

impl SyncMessage {
	pub fn header(&self) -> u8 {
		match self {
			Self::NewOperations => b'N',
			Self::OperationsRequest(_) => b'R',
			Self::OperationsRequestResponse(_) => b'P',
		}
	}

	pub async fn from_tunnel(stream: &mut Tunnel) -> std::io::Result<Self> {
		match stream.read_u8().await? {
			b'N' => Ok(Self::NewOperations),
			b'R' => Ok(Self::OperationsRequest(stream.read_u8().await?)),
			b'P' => Ok(Self::OperationsRequestResponse(stream.read_u8().await?)),
			header => Err(std::io::Error::new(
				std::io::ErrorKind::InvalidData,
				format!(
					"Invalid sync message header: {}",
					(header as char).to_string()
				),
			)),
		}
	}

	pub fn to_bytes(self, library_id: Uuid) -> Vec<u8> {
		let mut bytes = Header::Sync(library_id).to_bytes();
		bytes.push(self.header());

		match self {
			Self::OperationsRequest(s) => bytes.push(s),
			Self::OperationsRequestResponse(s) => bytes.push(s),
			_ => {}
		}

		bytes
	}
}
