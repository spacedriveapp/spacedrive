use std::{path::PathBuf, sync::Arc, time::Instant};

use sd_p2p::{Event, Manager, PeerId};
use sd_sync::CRDTOperation;
use tokio::{
	fs::File,
	io::{AsyncReadExt, AsyncWriteExt, BufReader},
};
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::{
	node::NodeConfigManager,
	p2p::{OperatingSystem, SPACEDRIVE_APP_ID},
};

use super::{Header, PeerMetadata};

pub struct P2PManager {
	manager: Arc<Manager<PeerMetadata>>,
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

		tokio::spawn(async move {
			while let Some(event) = stream.next().await {
				match event {
					Event::PeerDiscovered(event) => {
						debug!(
							"Discovered peer by id '{}' with address '{:?}' and metadata: {:?}",
							event.peer_id, event.addresses, event.metadata
						);
						event.dial().await; // We connect to everyone we find on the network. Your app will probs wanna restrict this!
					}
					Event::PeerMessage(mut event) => {
						tokio::spawn(async move {
							let header = Header::from_stream(&mut event.stream).await.unwrap();

							match header {
								Header::Ping => {
									debug!("Received ping from peer '{}'", event.peer_id);
								}
								Header::Spacedrop => {
									let file_length = event.stream.read_u8().await.unwrap();

									// TODO: Ask the user if they wanna reject/accept it

									info!("Received Spacedrop from peer '{}' with file length '{file_length}'", event.peer_id);

									let mut s = String::new();
									event.stream.read_to_string(&mut s).await.unwrap();

									// let mut buf = Vec::with_capacity(file_length as usize); // TODO: DOS attack
									// loop {
									// 	let n = event.stream.read(&mut buf).await.unwrap();;
									// }

									// // TODO: Store this to a file on disk.
									println!(
										"Recieved file content '{}' through Spacedrop!",
										s // String::from_utf8(buf).unwrap()
									);
								}
								Header::Sync(library_id) => {
									let buf_len = event.stream.read_u8().await.unwrap();

									let mut buf = vec![0; buf_len as usize]; // TODO: Designed for easily being able to be DOS the current Node
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

		let this = Arc::new(Self { manager });

		// TODO: Probs remove this
		tokio::spawn({
			let this = this.clone();
			async move {
				loop {
					tokio::time::sleep(std::time::Duration::from_secs(5)).await;
					this.ping().await;
				}
			}
		});

		// TODO: Probs remove this
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

		this
	}

	#[allow(unused)] // TODO: Remove `allow(unused)` once integrated
	pub async fn broadcast_sync_events(&self, library_id: Uuid, event: Vec<CRDTOperation>) {
		let mut head_buf = Header::Sync(library_id).to_bytes();
		let mut buf = rmp_serde::to_vec_named(&event).unwrap(); // TODO: Error handling
		head_buf.push(buf.len() as u8); // TODO: This is going to overflow quickly so deal with it properly!
		head_buf.append(&mut buf);

		self.manager.broadcast(buf).await;
	}

	pub async fn ping(&self) {
		self.manager.broadcast(Header::Ping.to_bytes()).await;
	}

	pub async fn big_bad_spacedrop(&self, peer_id: PeerId, path: PathBuf) {
		let mut stream = self.manager.stream(peer_id).await.unwrap(); // TODO: handle providing incorrect peer id

		let file = File::open(path).await.unwrap();
		let file_length = file.metadata().await.unwrap().len();
		let mut reader = BufReader::new(file);

		stream
			.write_all(&Header::Spacedrop.to_bytes()) // TODO: Proper Spaceblock Header
			.await
			.unwrap();

		stream
			.write_u8(file_length as u8) // TODO: This is obviously gonna be an int overflow. Fix that. Use `u64` in proper Spaceblock Header
			.await
			.unwrap();

		debug!("Starting Spacedrop to peer '{peer_id}'");
		let i = Instant::now();

		let mut buffer = Vec::new();
		reader.read_to_end(&mut buffer).await.unwrap();
		println!("READ {:?}", buffer);

		stream.write_all(&buffer).await.unwrap();

		// io::copy(&mut reader, &mut stream).await.unwrap(); // TODO: Use Spaceblock protocol!

		debug!(
			"Finished Spacedrop to peer '{peer_id}' after '{:?}",
			i.elapsed()
		);
	}
}
