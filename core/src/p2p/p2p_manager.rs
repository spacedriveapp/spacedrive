use std::{path::PathBuf, str::FromStr, sync::Arc, time::Instant};

use rspc::Type;
use sd_p2p::{
	spaceblock::{BlockSize, TransferRequest},
	Event, Manager, PeerId,
};
use sd_sync::CRDTOperation;
use serde::Serialize;
use tokio::{
	fs::File,
	io::{AsyncReadExt, AsyncWriteExt, BufReader},
	sync::broadcast,
};
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::{
	node::NodeConfigManager,
	p2p::{OperatingSystem, SPACEDRIVE_APP_ID},
};

use super::{Header, PeerMetadata};

/// TODO: P2P event for the frontend
#[derive(Debug, Clone, Type, Serialize)]
#[serde(tag = "type")]
pub enum P2PEvent {
	DiscoveredPeer {
		peer_id: PeerId,
		metadata: PeerMetadata,
	},
	// TODO: Expire peer + connection/disconnect
}

pub struct P2PManager {
	events: broadcast::Sender<P2PEvent>,
	// We hold this only so we don't get errors sending when no frontend's are listening
	_events_rx: broadcast::Receiver<P2PEvent>,
	pub manager: Arc<Manager<PeerMetadata>>,
}

impl P2PManager {
	pub async fn new(node_config: Arc<NodeConfigManager>) -> Arc<Self> {
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

		let (events_tx, events_rx) = broadcast::channel(100);
		let events = events_tx.clone();
		tokio::spawn(async move {
			while let Some(event) = stream.next().await {
				match event {
					Event::PeerDiscovered(event) => {
						debug!(
							"Discovered peer by id '{}' with address '{:?}' and metadata: {:?}",
							event.peer_id, event.addresses, event.metadata
						);

						events_tx
							.send(P2PEvent::DiscoveredPeer {
								peer_id: event.peer_id,
								metadata: event.metadata.clone(),
							})
							.map_err(|_| error!("Failed to send event to p2p event stream!"))
							.ok();

						// TODO: Don't just connect to everyone when we find them. We should only do it if we know them.
						event.dial().await;
					}
					Event::PeerMessage(mut event) => {
						tokio::spawn(async move {
							let header = Header::from_stream(&mut event.stream).await.unwrap();

							match header {
								Header::Ping => {
									debug!("Received ping from peer '{}'", event.peer_id);
								}
								Header::Spacedrop(req) => {
									info!("Received Spacedrop from peer '{}' for file '{}' with file length '{}'", event.peer_id, req.name, req.size);

									// TODO: Ask the user if they wanna reject/accept it

									// TODO: Deal with binary data. Deal with blocking based on `req.block_size`, etc
									let mut s = String::new();
									event.stream.read_to_string(&mut s).await.unwrap();

									println!(
										"Recieved file '{}' with content '{}' through Spacedrop!",
										req.name, s
									);

									// TODO: Save to the filesystem
								}
								Header::Sync(library_id) => {
									let mut len = [0; 4];
									event.stream.read_exact(&mut len).await.unwrap();
									let len = u32::from_be_bytes(len);

									let mut buf = vec![0; len as usize]; // TODO: Designed for easily being able to be DOS the current Node
									event.stream.read_exact(&mut buf).await.unwrap();

									let mut buf: &[u8] = &buf;
									let output: Vec<CRDTOperation> =
										rmp_serde::from_read(&mut buf).unwrap();

									// TODO: Handle this @Brendan
									println!("Received sync events for library '{library_id}': {output:?}");

									// TODO(@Oscar): Remember we can't do a response here cause it's a broadcast. Encode that into type system!
								}
							}
						});
					}
					_ => debug!("event: {:?}", event),
				}
			}

			error!("Manager event stream closed! The core is unstable from this point forward!");
		});

		// TODO: proper shutdown
		// https://docs.rs/ctrlc/latest/ctrlc/
		// https://docs.rs/system_shutdown/latest/system_shutdown/

		let this = Arc::new(Self {
			events,
			_events_rx: events_rx,
			manager,
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
			// 			this.big_bad_spacedrop(peer_id, PathBuf::from("./demo.txt"))
			// 				.await;
			// 		} else {
			// 			info!("No clients found so skipping Spacedrop demo!");
			// 		}
			// 	}
			// });

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
						this.broadcast_sync_events(
							Uuid::from_str("e4372586-d028-48f8-8be6-b4ff781a7dc2").unwrap(),
							vec![],
						)
						.await;
					} else {
						info!("No clients found so skipping Spacedrop demo!");
					}
				}
			});
		}

		this
	}

	pub fn subscribe(&self) -> broadcast::Receiver<P2PEvent> {
		self.events.subscribe()
	}

	#[allow(unused)] // TODO: Remove `allow(unused)` once integrated
	pub async fn broadcast_sync_events(&self, library_id: Uuid, event: Vec<CRDTOperation>) {
		let mut head_buf = Header::Sync(library_id).to_bytes();
		let mut buf = rmp_serde::to_vec_named(&event).unwrap(); // TODO: Error handling

		let len: u32 = buf.len().try_into().unwrap(); // Max Sync payload is like 4GB
		let mut len_buf = len.to_le_bytes();
		debug_assert_eq!(len_buf.len(), 4);

		head_buf.extend_from_slice(&len_buf);
		head_buf.append(&mut buf);

		self.manager.broadcast(head_buf).await;
	}

	pub async fn ping(&self) {
		self.manager.broadcast(Header::Ping.to_bytes()).await;
	}

	pub async fn big_bad_spacedrop(&self, peer_id: PeerId, path: PathBuf) {
		let mut stream = self.manager.stream(peer_id).await.unwrap(); // TODO: handle providing incorrect peer id

		let file = File::open(&path).await.unwrap();
		let metadata = file.metadata().await.unwrap();
		let mut reader = BufReader::new(file);

		stream
			.write_all(
				&Header::Spacedrop(TransferRequest {
					name: path.file_name().unwrap().to_str().unwrap().to_string(), // TODO: Encode this as bytes instead
					size: metadata.len(),
					block_size: BlockSize::from_size(metadata.len()),
				})
				.to_bytes(),
			)
			.await
			.unwrap();

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
