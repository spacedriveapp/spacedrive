use std::{collections::HashMap, env, fs::File, pin::Pin, sync::Arc};

use futures::{executor::block_on, Future};
use p2p::{
	quinn::{RecvStream, SendStream},
	Identity, NetworkManager, NetworkManagerConfig, NetworkManagerError, P2PManager, Peer, PeerId,
	PeerMetadata,
};
use tokio::sync::mpsc::{self};
use uuid::Uuid;

use crate::{
	library::{LibraryConfig, LibraryContext, LibraryManager},
	node::NodeConfigManager,
	ClientQuery, CoreEvent, CoreResponse,
};

use super::{P2PRequest, P2PResponse};

const LIBRARY_ID_EXTRA_DATA_KEY: &'static str = "libraryId";
const LIBRARY_CONFIG_EXTRA_DATA_KEY: &'static str = "libraryData";

#[derive(Debug, Clone)]
pub enum P2PEvent {
	PeerDiscovered(PeerId),
	PeerExpired(PeerId),
	PeerConnected(PeerId),
	PeerDisconnected(PeerId),
}

// SdP2PManager is part of your application and allows you to hook into the behavior of the P2PManager.
#[derive(Clone)]
pub struct SdP2PManager {
	library_manager: Arc<LibraryManager>,
	config: Arc<NodeConfigManager>,
	/// event_channel is used to send events back to the Spacedrive main event loop
	event_channel: mpsc::UnboundedSender<P2PEvent>,
}

impl P2PManager for SdP2PManager {
	const APPLICATION_NAME: &'static str = "spacedrive";

	fn get_metadata(&self) -> PeerMetadata {
		PeerMetadata {
			// TODO: `block_on` needs to be removed from here!
			name: block_on(self.config.get()).name.clone(),
			version: Some(env!("CARGO_PKG_VERSION").into()),
		}
	}

	fn peer_discovered(&self, nm: &NetworkManager<Self>, peer_id: &PeerId) {
		self.event_channel
			.send(P2PEvent::PeerDiscovered(peer_id.clone()));
		// nm.add_known_peer(peer_id.clone()); // Be careful doing this in a production application because it will just trust all clients
	}

	fn peer_expired(&self, nm: &NetworkManager<Self>, peer_id: PeerId) {
		self.event_channel.send(P2PEvent::PeerExpired(peer_id));
	}

	fn peer_connected(&self, nm: &NetworkManager<Self>, peer_id: PeerId) {
		self.event_channel.send(P2PEvent::PeerConnected(peer_id));
	}

	fn peer_disconnected(&self, nm: &NetworkManager<Self>, peer_id: PeerId) {
		self.event_channel.send(P2PEvent::PeerDisconnected(peer_id));
	}

	fn peer_paired<'a>(
		&'a self,
		nm: &'a NetworkManager<Self>,
		peer_id: &'a PeerId,
		extra_data: &'a HashMap<String, String>,
	) -> Pin<Box<dyn Future<Output = Result<(), ()>> + Send + 'a>> {
		// TODO: Checking is peer is the same or newer version of application and hence that it's safe to join

		Box::pin(async move {
			let library_id = extra_data.get(LIBRARY_ID_EXTRA_DATA_KEY).unwrap();
			let library_config: LibraryConfig =
				serde_json::from_str(extra_data.get(LIBRARY_CONFIG_EXTRA_DATA_KEY).unwrap())
					.unwrap();

			let ctx = self
				.library_manager
				.create_with_id(Uuid::parse_str(library_id).unwrap(), library_config)
				.await
				.unwrap();

			// TODO: Create clients in the DB -> The first client should send over all the data

			// TODO: Emit InvalidQuery events

			Ok(())
		})
	}

	fn peer_paired_rollback<'a>(
		&'a self,
		nm: &'a NetworkManager<Self>,
		peer_id: &'a PeerId,
		extra_data: &'a HashMap<String, String>,
	) -> Pin<Box<dyn Future<Output = ()> + Send + Sync + 'a>> {
		Box::pin(async move {
			println!("TODO: Rolling back changes from `peer_paired` as connection failed.");

			// TODO: Undo DB changes

			// TODO: Emit `InvalidateQuery` events
		})
	}

	fn accept_stream(&self, peer: &Peer<Self>, (mut tx, mut rx): (SendStream, RecvStream)) {
		let peer = peer.clone();
		tokio::spawn(async move {
			// TODO: Get max length from constant.
			let msg = rx.read_chunk(1024, true).await.unwrap().unwrap();
			let req: P2PRequest = rmp_serde::from_slice(&msg.bytes).unwrap();

			match req {
				P2PRequest::Ping => {
					println!("Received ping from '{}'", peer.id);
					tx.write(&rmp_serde::encode::to_vec_named(&P2PResponse::Pong).unwrap())
						.await
						.unwrap();
				}
				P2PRequest::GetFile { path } => {
					println!("Sending file at path '{}'", path);

					// tokio::fs::read(&filename).unwrap();

					// match File::open(&filename) {
					// 	Ok(mut file) => {
					// 		// file

					// 		// let mut buf = match fs::metadata(&filename) {
					// 		// 	Ok(metadata) => Vec::with_capacity(metadata.len() as usize),
					// 		// 	Err(_) => Vec::new(),
					// 		// };

					// 		// file.read_to_end(&mut buf).unwrap();
					// 	}
					// 	Err(Error) => {
					// 		println!("{}", err);
					// 		todo!();
					// 	}
					// }
				}
			}
		});
	}
}

pub async fn init(
	library_manager: Arc<LibraryManager>,
	config: Arc<NodeConfigManager>,
) -> Result<
	(
		Arc<NetworkManager<SdP2PManager>>,
		mpsc::UnboundedReceiver<P2PEvent>,
	),
	NetworkManagerError,
> {
	let identity = Identity::new().unwrap(); // TODO: Save and load from Spacedrive config
	let event_channel = mpsc::unbounded_channel();
	let nm = NetworkManager::new(
		identity,
		SdP2PManager {
			library_manager,
			config,
			event_channel: event_channel.0,
		},
		NetworkManagerConfig {
			known_peers: Default::default(), // TODO: Load these from the database on startup
			listen_port: None,
		},
	)
	.await?;
	println!(
		"Peer '{}' listening on: {:?}",
		nm.peer_id(),
		nm.listen_addr()
	);

	// TODO: abstraction for this
	// let (mut tx, mut rx) = nm.stream(peer_id).await.unwrap();
	// tx.write_all(rmp_serde::encode::to_vec_named(&P2PRequest::Ping))
	// 	.await
	// 	.unwrap();

	Ok((nm, event_channel.1))
}

pub async fn pair(
	nm: &Arc<NetworkManager<SdP2PManager>>,
	ctx: LibraryContext,
	peer_id: PeerId,
) -> CoreResponse {
	nm.clone()
		.pair_with_peer(
			peer_id,
			[
				(LIBRARY_ID_EXTRA_DATA_KEY.into(), ctx.id.to_string()),
				(
					LIBRARY_CONFIG_EXTRA_DATA_KEY.into(),
					serde_json::to_string(&ctx.config).unwrap(),
				),
			]
			.into_iter()
			.collect(),
		)
		.await;

	let password = "todo".to_string(); // TODO: make this work with UI

	// TODO: Add node into library database once paired

	// TODO: Integrate proper pairing protocol using PAKE system to ensure we trust the remote client.
	// nm.add_known_peer(peer_id);

	ctx.emit(CoreEvent::InvalidateQuery(ClientQuery::DiscoveredPeers))
		.await;
	ctx.emit(CoreEvent::InvalidateQuery(ClientQuery::ConnectedPeers))
		.await;

	CoreResponse::PairNode { password }
}
