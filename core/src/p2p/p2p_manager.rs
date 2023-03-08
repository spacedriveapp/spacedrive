use std::sync::Arc;

use sd_p2p::{Event, Manager};
use sd_sync::CRDTOperation;
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::{
	library::LibraryManager,
	node::NodeConfigManager,
	p2p::{OperatingSystem, SPACEDRIVE_APP_ID},
};

use super::{Header, PeerMetadata};

pub struct P2PManager {
	manager: Arc<Manager<PeerMetadata>>,
}

impl P2PManager {
	pub async fn new(
		node_config: Arc<NodeConfigManager>,
		library_manager: Arc<LibraryManager>,
	) -> Arc<Self> {
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
				config.keypair.clone(),
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
									todo!();
								}
								Header::Sync(library_id) => {
									let buf_len = event.stream.read_u8().await.unwrap();

									let mut buf = Vec::with_capacity(buf_len as usize); // TODO: Designed for easily being able to be DOS the current Node
									event.stream.read_exact(&mut buf).await.unwrap();

									let mut buf: &[u8] = &buf;
									let output: Vec<CRDTOperation> =
										rmp_serde::from_read(&mut buf).unwrap();

									// TODO: Handle this @Brendan
									println!("Receieved sync events for library '{library_id}': {output:?}");

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

		this
	}

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
}
