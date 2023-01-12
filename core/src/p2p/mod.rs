use std::{sync::Arc, time::Duration};

use sd_p2p::{Event, Manager};
use sd_sync::CRDTOperation;
use tokio::{sync::mpsc, time::sleep};
use tracing::info;
use uuid::Uuid;

use crate::{invalidate_query, library::LibraryManager, node::NodeConfigManager};

use self::peer_metadata::PeerMetadata;

mod peer_metadata;

const SPACEDRIVE_APP_ID: &'static str = "spacedrive";

#[derive(Clone, Debug)]
pub enum P2PEvent {
	// TODO
}

/// TODO
#[derive(Debug, Clone)]
pub struct PeerBootstrapProgress {
	synced_rows: u128,
	total_rows: u128,
}

pub struct P2PManager {}

impl P2PManager {
	pub async fn new(
		node_config: Arc<NodeConfigManager>,
		library_manager: Arc<LibraryManager>,
		mut p2p_rx: mpsc::Receiver<(Uuid, CRDTOperation)>,
	) -> Arc<Self> {
		let config = Arc::new(node_config.get().await); // TODO: Update this throughout the application lifecycle

		let this = Arc::new(Self {});

		let manager = Manager::new(
			SPACEDRIVE_APP_ID,
			&config
				.keypair
				.as_ref()
				.expect("Keypair not found. This should be unreachable code!"),
			move || async move {
				PeerMetadata {
					name: "123".to_string(), // config.name.clone(), // TODO
				}
			},
			|manager, event| async move {
				// TODO: Send all these events to frontend through rspc
				match event {
					Event::PeerDiscovered(event) => {
						println!(
							"Discovered peer by id '{}' with address '{:?}' and metadata: {:?}",
							event.peer_id(),
							event.addresses(),
							event.metadata()
						);

						// TODO: Tie this into Spacedrive
						event.dial(&manager).await;
					}
					event => println!("{:?}", event),
				}
			},
			// This closure it run to handle a single incoming request. It's return type is then sent back to the client.
			// TODO: Why can't it infer the second param here???
			{
				let library_manager = library_manager.clone();
				move |_manager, data: Vec<u8>| {
					let library_manager = library_manager.clone(); // This makes sure this function is `Fn` not `FnOnce`.
					async move {
						if data.len() == 4
							&& data[0] == 0 && data[1] == 1
							&& data[2] == 2 && data[3] == 3
						{
							println!("Received ping!");
							return Ok(vec![0, 1, 2, 3]); // TODO: Being empty breaks shit
						}

						let (library_id, op) =
							rmp_serde::from_slice::<(Uuid, CRDTOperation)>(&data).unwrap();
						println!(
							"P2P Received Sync Operations for library '{}': {:?}",
							library_id, op
						);

						let ctx = library_manager.get_ctx(library_id).await.unwrap();

						ctx.sync.ingest_op(op).await.unwrap();

						invalidate_query!(ctx, "locations.list"); // TODO: Brendan's sync system needs to handle data invalidation

						Ok(vec![0, 1, 2, 3]) // TODO: Being empty breaks shit
					}
				}
			},
		)
		.await
		.unwrap();

		tokio::spawn({
			let this = this.clone();
			let manager = manager.clone();
			async move {
				while let Some(op) = p2p_rx.recv().await {
					// TODO: Only seen to peers in the current library and deal with library signing here.
					// TODO: Put protocol above broadcast feature.
					manager
						.broadcast(&rmp_serde::to_vec_named(&op).unwrap())
						.await;
				}
			}
		});

		tokio::spawn(async move {
			sleep(Duration::from_millis(500)).await;
			info!(
				"Node '{}' is now online listening at addresses: {:?}",
				manager.peer_id(),
				manager.listen_addrs().await
			);

			// TODO: Remove this without the connections timing out????
			loop {
				sleep(Duration::from_secs(3)).await;
				manager.clone().broadcast(&[0, 1, 2, 3]).await;
				// println!("Sent broadcast!");
			}
		});

		// TODO: proper shutdown
		// https://docs.rs/ctrlc/latest/ctrlc/
		// https://docs.rs/system_shutdown/latest/system_shutdown/

		this
	}

	pub fn mount_library(&self) {}
}
