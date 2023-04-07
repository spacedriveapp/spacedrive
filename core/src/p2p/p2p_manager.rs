use std::{
	collections::HashMap,
	path::PathBuf,
	sync::Arc,
	time::{Duration, Instant},
};

use rspc::Type;
use sd_p2p::{
	spaceblock::{BlockSize, SpacedropRequest},
	spacetime::SpaceTimeStream,
	Event, Manager, PeerId,
};
use sd_sync::CRDTOperation;
use serde::Serialize;
use tokio::{
	fs::File,
	io::{self, AsyncReadExt, AsyncWriteExt, BufReader},
	sync::{broadcast, oneshot, Mutex},
	time::sleep,
};
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::{
	node::NodeConfigManager,
	p2p::{OperatingSystem, SPACEDRIVE_APP_ID},
};

use super::{Header, PeerMetadata};

/// The amount of time to wait for a Spacedrop request to be accepted or rejected before it's automatically rejected
const SPACEDROP_TIMEOUT: Duration = Duration::from_secs(60);

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
	events: broadcast::Sender<P2PEvent>,
	pub manager: Arc<Manager<PeerMetadata>>,
	spacedrop_pairing_reqs: Arc<Mutex<HashMap<Uuid, oneshot::Sender<Option<String>>>>>,
}

impl P2PManager {
	pub async fn new(
		node_config: Arc<NodeConfigManager>,
	) -> (Arc<Self>, broadcast::Receiver<(Uuid, Vec<CRDTOperation>)>) {
		let (config, keypair) = {
			let config = node_config.get().await;
			(
				PeerMetadata {
					name: config.name.clone(),
					operating_system: Some(OperatingSystem::get_os()),
					version: Some(env!("CARGO_PKG_VERSION").to_string()),
					email: config.p2p_email.clone(),
					img_url: config.p2p_img_url.clone(),
				},
				config.keypair,
			)
		}; // TODO: Update this throughout the application lifecycle

		let (manager, mut stream) = Manager::new(SPACEDRIVE_APP_ID, &keypair, {
			move || {
				let config = config.clone();
				async move { config }
			}
		})
		.await
		.unwrap();

		info!(
			"Node '{}' is now online listening at addresses: {:?}",
			manager.peer_id(),
			manager.listen_addrs().await
		);

		let (tx, rx) = broadcast::channel(100);
		let (tx2, rx2) = broadcast::channel(100);

		let spacedrop_pairing_reqs = Arc::new(Mutex::new(HashMap::new()));
		tokio::spawn({
			let events = tx.clone();
			let sync_events = tx2.clone();
			let spacedrop_pairing_reqs = spacedrop_pairing_reqs.clone();

			async move {
				while let Some(event) = stream.next().await {
					match event {
						Event::PeerDiscovered(event) => {
							debug!(
								"Discovered peer by id '{}' with address '{:?}' and metadata: {:?}",
								event.peer_id, event.addresses, event.metadata
							);

							events
								.send(P2PEvent::DiscoveredPeer {
									peer_id: event.peer_id.clone(),
									metadata: event.metadata.clone(),
								})
								.map_err(|_| error!("Failed to send event to p2p event stream!"))
								.ok();

							// TODO: Don't just connect to everyone when we find them. We should only do it if we know them.
							event.dial().await;
						}
						Event::PeerMessage(mut event) => {
							let events = events.clone();
							let sync_events = sync_events.clone();
							let spacedrop_pairing_reqs = spacedrop_pairing_reqs.clone();

							// TODO: Prevent accepting too many requests quickly and killing tokio. Restrict number of unauthenticated reqs.
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
										events
											.send(P2PEvent::SpacedropRequest {
												id,
												peer_id: event.peer_id,
												name: req.name,
											})
											.unwrap();

										let file_path = tokio::select! {
											_ = sleep(SPACEDROP_TIMEOUT) => {
												info!("spacedrop({id}): timeout, rejecting!");

												return;
											}
											file_path = rx => {
												match file_path {
													Ok(Some(file_path)) => {
														info!("spacedrop({id}): accepted saving to '{:?}'", file_path);
														file_path
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

										stream.write_all(&[1]).await.unwrap();

										let mut f = File::create(file_path).await.unwrap();

										// TODO: Use binary block protocol instead of this
										io::copy(&mut stream, &mut f).await.unwrap();

										info!("spacedrop({id}): complete");
									}
									Header::Sync(library_id, len) => {
										let mut buf = vec![0; len as usize]; // TODO: Designed for easily being able to be DOS the current Node
										event.stream.read_exact(&mut buf).await.unwrap();

										let mut buf: &[u8] = &buf;
										let operations = rmp_serde::from_read(&mut buf).unwrap();

										println!("Received sync events for library '{library_id}': {operations:?}");

										sync_events.send((library_id, operations)).unwrap();
									}
								}
							});
						}
						_ => debug!("event: {:?}", event),
					}
				}

				error!(
					"Manager event stream closed! The core is unstable from this point forward!"
				);
			}
		});

		// TODO: proper shutdown
		// https://docs.rs/ctrlc/latest/ctrlc/
		// https://docs.rs/system_shutdown/latest/system_shutdown/

		let this = Arc::new(Self {
			events: tx,
			manager,
			spacedrop_pairing_reqs,
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

		// TODO(@Oscar): Remove this in the future once i'm done using it for testing
		if std::env::var("SPACEDROP_DEMO").is_ok() {
			tokio::spawn({
				let this = this.clone();
				async move {
					tokio::time::sleep(std::time::Duration::from_secs(5)).await;
					let mut connected = this
						.manager
						.get_connected_peers()
						.await
						.unwrap()
						.into_iter();
					if let Some(peer_id) = connected.next() {
						info!("Starting Spacedrop to peer '{}'", peer_id);
						this.big_bad_spacedrop(peer_id, PathBuf::from("./demo.txt"))
							.await;
					} else {
						info!("No clients found so skipping Spacedrop demo!");
					}
				}
			});

			// tokio::spawn({
			// 	let this = this.clone();
			// 	async move {
			// 		tokio::time::sleep(std::time::Duration::from_secs(5)).await;
			// 		let mut connected = this
			// 			.manager
			// 			.get_connected_peers()
			// 			.await
			// 			.unwrap()
			// 			.into_iter();
			// 		if let Some(peer_id) = connected.next() {
			// 			info!("Starting Spacedrop to peer '{}'", peer_id);
			// 			this.broadcast_sync_events(
			// 				Uuid::from_str("e4372586-d028-48f8-8be6-b4ff781a7dc2").unwrap(),
			// 				vec![CRDTOperation {
			// 					node: Uuid::new_v4(),
			// 					timestamp: NTP64(1),
			// 					id: Uuid::new_v4(),
			// 					typ: CRDTOperationType::Owned(OwnedOperation {
			// 						model: "TODO".to_owned(),
			// 						items: Vec::new(),
			// 					}),
			// 				}],
			// 			)
			// 			.await;
			// 		} else {
			// 			info!("No clients found so skipping Spacedrop demo!");
			// 		}
			// 	}
			// });
		}

		(this, rx2)
	}

	pub fn subscribe(&self) -> broadcast::Receiver<P2PEvent> {
		self.events.subscribe()
	}

	#[allow(unused)] // TODO: Remove `allow(unused)` once integrated
	pub async fn broadcast_sync_events(&self, library_id: Uuid, event: Vec<CRDTOperation>) {
		let mut buf = rmp_serde::to_vec_named(&event).unwrap(); // TODO: Error handling
		let mut head_buf = Header::Sync(library_id, buf.len() as u32).to_bytes(); // Max Sync payload is like 4GB
		head_buf.append(&mut buf);

		debug!("broadcasting sync events. payload_len={}", buf.len());

		self.manager.broadcast(head_buf).await;
	}

	pub async fn ping(&self) {
		self.manager.broadcast(Header::Ping.to_bytes()).await;
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

	pub async fn big_bad_spacedrop(&self, peer_id: PeerId, path: PathBuf) {
		let mut stream = self.manager.stream(peer_id).await.unwrap(); // TODO: handle providing incorrect peer id

		let file = File::open(&path).await.unwrap();
		let metadata = file.metadata().await.unwrap();
		let mut reader = BufReader::new(file);

		stream
			.write_all(
				&Header::Spacedrop(SpacedropRequest {
					name: path.file_name().unwrap().to_str().unwrap().to_string(), // TODO: Encode this as bytes instead
					size: metadata.len(),
					block_size: BlockSize::from_size(metadata.len()),
				})
				.to_bytes(),
			)
			.await
			.unwrap();

		debug!("Waiting for Spacedrop to be accepted from peer '{peer_id}'");
		let mut buf = [0; 1];
		// TODO: Add timeout so the connection is dropped if they never response
		stream.read_exact(&mut buf).await.unwrap();
		if buf[0] != 1 {
			debug!("Spacedrop was rejected from peer '{peer_id}'");
			return;
		}

		debug!("Starting Spacedrop to peer '{peer_id}'");
		let i = Instant::now();

		// TODO: Replace this with the Spaceblock `Block` system
		let mut buffer = Vec::new();
		reader.read_to_end(&mut buffer).await.unwrap();
		println!("READ {:?}", buffer);
		stream.write_all(&buffer).await.unwrap();

		debug!(
			"Finished Spacedrop to peer '{peer_id}' after '{:?}",
			i.elapsed()
		);
	}
}
