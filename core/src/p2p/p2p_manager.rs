#![allow(clippy::unwrap_used)] // TODO: Remove once this is fully stablised

use std::{
	borrow::Cow,
	collections::HashMap,
	path::PathBuf,
	sync::{
		atomic::{AtomicU16, Ordering},
		Arc,
	},
	time::{Duration, Instant},
};

use futures::Stream;
use sd_p2p::{
	spaceblock::{BlockSize, SpacedropRequest, Transfer},
	spacetime::SpaceTimeStream,
	spacetunnel::{Identity, RemoteIdentity, Tunnel},
	Event, Manager, ManagerError, MetadataManager, PeerId,
};
use sd_sync::CRDTOperation;
use serde::Serialize;
use specta::Type;
use tokio::{
	fs::File,
	io::{AsyncReadExt, AsyncWriteExt, BufReader},
	sync::{broadcast, oneshot, Mutex},
	time::sleep,
};
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::{
	library::Library,
	node::{NodeConfig, NodeConfigManager},
	p2p::{OperatingSystem, SPACEDRIVE_APP_ID},
};

use super::{Header, PeerMetadata};

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
	// TODO: Expire peer + connection/disconnect
}

pub struct P2PManager {
	pub events: (broadcast::Sender<P2PEvent>, broadcast::Receiver<P2PEvent>),
	pub manager: Arc<Manager<PeerMetadata>>,
	spacedrop_pairing_reqs: Arc<Mutex<HashMap<Uuid, oneshot::Sender<Option<String>>>>>,
	pub metadata_manager: Arc<MetadataManager<PeerMetadata>>,
	pub spacedrop_progress: Arc<Mutex<HashMap<Uuid, broadcast::Sender<u8>>>>,
	pairing_id: AtomicU16,
}

impl P2PManager {
	pub async fn new(
		node_config: Arc<NodeConfigManager>,
	) -> Result<(Arc<Self>, broadcast::Receiver<(Uuid, Vec<CRDTOperation>)>), ManagerError> {
		let (config, keypair) = {
			let config = node_config.get().await;
			(Self::config_to_metadata(&config), config.keypair)
		};

		let metadata_manager = MetadataManager::new(config);

		let (manager, mut stream) =
			Manager::new(SPACEDRIVE_APP_ID, &keypair, metadata_manager.clone()).await?;

		info!(
			"Node '{}' is now online listening at addresses: {:?}",
			manager.peer_id(),
			manager.listen_addrs().await
		);

		// need to keep 'rx' around so that the channel isn't dropped
		let (tx, rx) = broadcast::channel(100);
		let (tx2, rx2) = broadcast::channel(100);

		let spacedrop_pairing_reqs = Arc::new(Mutex::new(HashMap::new()));
		let spacedrop_progress = Arc::new(Mutex::new(HashMap::new()));

		tokio::spawn({
			let events = tx.clone();
			// let sync_events = tx2.clone();
			let spacedrop_pairing_reqs = spacedrop_pairing_reqs.clone();
			let spacedrop_progress = spacedrop_progress.clone();

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
						Event::PeerMessage(mut event) => {
							let events = events.clone();
							let sync_events = tx2.clone();
							let spacedrop_pairing_reqs = spacedrop_pairing_reqs.clone();
							let spacedrop_progress = spacedrop_progress.clone();

							tokio::spawn(async move {
								let header = Header::from_stream(&mut event.stream).await.unwrap();

								match header {
									Header::Ping => {
										debug!("Received ping from peer '{}'", event.peer_id);
									}
									Header::Spacedrop(req) => {
										let mut stream = match event.stream {
											SpaceTimeStream::Unicast(stream) => stream,
											_ => {
												// TODO: Return an error to the remote client
												error!("Received Spacedrop request from peer '{}' but it's not a unicast stream!", event.peer_id);
												return;
											}
										};
										let id = Uuid::new_v4();
										let (tx, rx) = oneshot::channel();

										info!("spacedrop({id}): received from peer '{}' for file '{}' with file length '{}'", event.peer_id, req.name, req.size);

										spacedrop_pairing_reqs.lock().await.insert(id, tx);

										let (process_tx, _) = broadcast::channel(100);
										spacedrop_progress
											.lock()
											.await
											.insert(id, process_tx.clone());

										if let Err(_) = events.send(P2PEvent::SpacedropRequest {
											id,
											peer_id: event.peer_id,
											name: req.name.clone(),
										}) {
											// No frontend's are active

											todo!("Outright reject Spacedrop");
										}

										tokio::select! {
											_ = sleep(SPACEDROP_TIMEOUT) => {
												info!("spacedrop({id}): timeout, rejecting!");

												return;
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
														return;
													}
													Err(_) => {
														info!("spacedrop({id}): error with Spacedrop pairing request receiver!");
														return;
													}
												}
											}
										};
									}
									Header::Pair(library_id) => {
										info!(
											"Starting pairing with '{}' for library '{library_id}'",
											event.peer_id
										);

										// TODO: Security stuff

										let public_key = {
											// TODO: Prevent DOS
											let len = event.stream.read_u16_le().await.unwrap();
											let mut buf = vec![0; len as usize];
											let data =
												event.stream.read_exact(&mut buf).await.unwrap();
											RemoteIdentity::from_bytes(&buf).unwrap()
										};

										// TODO: Put remove node into the local DB
									}
									Header::Sync(library_id) => {
										let tunnel =
											Tunnel::from_stream(event.stream).await.unwrap();

										todo!();

										// let mut len = [0; 4];
										// stream
										// 	.read_exact(&mut len)
										// 	.await
										// 	.map_err(SyncRequestError::PayloadLenIoError)?;
										// let len = u32::from_le_bytes(len);

										// let mut buf = vec![0; len as usize]; // TODO: Designed for easily being able to be DOS the current Node
										// event.stream.read_exact(&mut buf).await.unwrap();

										// let mut buf: &[u8] = &buf;
										// let operations = rmp_serde::from_read(&mut buf).unwrap();

										// println!("Received sync events for library '{library_id}': {operations:?}");

										// sync_events.send((library_id, operations)).unwrap();
									}
								}
							});
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

		// TODO: proper shutdown
		// https://docs.rs/ctrlc/latest/ctrlc/
		// https://docs.rs/system_shutdown/latest/system_shutdown/

		let this = Arc::new(Self {
			events: (tx, rx),
			manager,
			spacedrop_pairing_reqs,
			metadata_manager,
			spacedrop_progress,
			pairing_id: AtomicU16::new(0),
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

		Ok((this, rx2))
	}

	fn config_to_metadata(config: &NodeConfig) -> PeerMetadata {
		PeerMetadata {
			name: config.name.clone(),
			operating_system: Some(OperatingSystem::get_os()),
			version: Some(env!("CARGO_PKG_VERSION").to_string()),
			email: config.p2p_email.clone(),
			img_url: config.p2p_img_url.clone(),
		}
	}

	pub async fn update_metadata(&self, node_config_manager: &NodeConfigManager) {
		self.metadata_manager
			.update(Self::config_to_metadata(&node_config_manager.get().await));
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

	pub fn pair(&self, peer_id: PeerId, lib: Library) -> u16 {
		let pairing_id = self.pairing_id.fetch_add(1, Ordering::SeqCst);

		let manager = self.manager.clone();
		tokio::spawn(async move {
			info!(
				"Started pairing session '{pairing_id}' with peer '{peer_id}' for library '{}'",
				lib.id
			);

			let mut stream = manager.stream(peer_id).await.unwrap();

			let header = Header::Pair(lib.id);
			stream.write_all(&header.to_bytes()).await.unwrap();

			// TODO: Apply some security here cause this is so open to MITM
			// TODO: Signing and a SPAKE style pin prompt

			let public_key = lib.identity.public_key();
			let public_key = public_key.to_bytes();
			stream
				.write_all(&public_key.len().to_le_bytes())
				.await
				.unwrap();
			stream.write_all(&public_key).await.unwrap();

			// TODO: Send nodeinfo

			// TODO: Recieve nodeinfo

			// lib.db.node().create(pub_id, name, params);

			// TODO: Add remote node into local DB
		});

		pairing_id
	}

	pub async fn broadcast_sync_events(
		&self,
		library_id: Uuid,
		identity: &Identity,
		event: Vec<CRDTOperation>,
	) {
		let mut buf = match rmp_serde::to_vec_named(&event) {
			Ok(buf) => buf,
			Err(e) => {
				error!("Failed to serialize sync event: {:?}", e);
				return;
			}
		};
		let mut head_buf = Header::Sync(library_id).to_bytes(); // Max Sync payload is like 4GB
		head_buf.append(&mut buf);

		// TODO: Determine which clients we share that library with

		// TODO: Establish a connection to them

		// TODO: Use `Tunnel` for encryption

		todo!();

		// buf.len() as u32

		// let len_buf = len.to_le_bytes();
		// debug_assert_eq!(len_buf.len(), 4); // TODO: Is this bad because `len` is usize??
		// bytes.extend_from_slice(&len_buf);

		// debug!("broadcasting sync events. payload_len={}", buf.len());

		// self.manager.broadcast(head_buf).await;
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

		let header = Header::Spacedrop(SpacedropRequest {
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
