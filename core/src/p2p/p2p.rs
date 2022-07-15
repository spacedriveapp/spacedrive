use std::{
	collections::HashMap,
	env,
	pin::Pin,
	str::FromStr,
	sync::{Arc, Mutex},
};

use futures::{executor::block_on, Future};
use p2p::{
	quinn::{RecvStream, SendStream},
	read_value, write_value, Identity, NetworkManager, NetworkManagerConfig, NetworkManagerError,
	OperationSystem, P2PManager, PairingParticipantType, Peer, PeerId, PeerMetadata,
};
use tokio::sync::{
	mpsc::{self},
	oneshot,
};
use tracing::{error, info};
use uuid::Uuid;

use crate::{
	library::{LibraryConfig, LibraryContext, LibraryManager},
	node::NodeConfigManager,
	prisma::node,
	ClientQuery, CoreEvent, CoreResponse,
};

use super::{P2PRequest, P2PResponse};

// TODO: Disable IPv6 record being advertised via DNS "tunnel.spacedrive.com:443"; // TODO: This should be on port 443
pub const SPACETUNNEL_URL: &'static str = "213.188.211.127:9000"; // TODO: Disable IPv6 record being advertised via DNS "tunnel.spacedrive.com:443"; // TODO: This should be on port 443

const LIBRARY_ID_EXTRA_DATA_KEY: &'static str = "libraryId";
const LIBRARY_CONFIG_EXTRA_DATA_KEY: &'static str = "libraryData";

#[derive(Debug, Clone)]
pub enum P2PEvent {
	PeerDiscovered(PeerId),
	PeerExpired(PeerId),
	PeerConnected(PeerId),
	PeerDisconnected(PeerId),
	PeerPairingRequest {
		peer_id: PeerId,
		peer_metadata: PeerMetadata,
		library_id: Uuid,
	},
	PeerPairingComplete {
		peer_id: PeerId,
		peer_metadata: PeerMetadata,
		library_id: Uuid,
	},
}

// TODO: rename this
pub struct P2PData {
	pub nm: Arc<NetworkManager<SdP2PManager>>,
	pub event_receiver: mpsc::UnboundedReceiver<P2PEvent>,
	pub pairing_requests: Arc<Mutex<HashMap<PeerId, oneshot::Sender<Result<String, ()>>>>>,
}

// SdP2PManager is part of your application and allows you to hook into the behavior of the P2PManager.
#[derive(Clone)]
pub struct SdP2PManager {
	library_manager: Arc<LibraryManager>,
	config: Arc<NodeConfigManager>,
	/// event_channel is used to send events back to the Spacedrive main event loop
	event_channel: mpsc::UnboundedSender<P2PEvent>,
	pairing_requests: Arc<Mutex<HashMap<PeerId, oneshot::Sender<Result<String, ()>>>>>,
}

impl P2PManager for SdP2PManager {
	const APPLICATION_NAME: &'static str = "spacedrive";

	fn get_metadata(&self) -> PeerMetadata {
		PeerMetadata {
			// TODO: `block_on` needs to be removed from here!
			name: block_on(self.config.get()).name.clone(),
			operating_system: Some(OperationSystem::get_os()),
			version: Some(env!("CARGO_PKG_VERSION").into()),
		}
	}

	fn peer_discovered(&self, _nm: &NetworkManager<Self>, peer_id: &PeerId) {
		match self
			.event_channel
			.send(P2PEvent::PeerDiscovered(peer_id.clone()))
		{
			Ok(_) => (),
			Err(err) => error!("Error sending P2PEvent::PeerDiscovered: {}", err),
		}
	}

	fn peer_expired(&self, _nm: &NetworkManager<Self>, peer_id: PeerId) {
		match self.event_channel.send(P2PEvent::PeerExpired(peer_id)) {
			Ok(_) => (),
			Err(err) => error!("Error sending P2PEvent::PeerExpired: {}", err),
		}
	}

	fn peer_connected(&self, _nm: &NetworkManager<Self>, peer_id: PeerId) {
		match self.event_channel.send(P2PEvent::PeerConnected(peer_id)) {
			Ok(_) => (),
			Err(err) => error!("Error sending P2PEvent::PeerConnected: {}", err),
		}
	}

	fn peer_disconnected(&self, _nm: &NetworkManager<Self>, peer_id: PeerId) {
		match self.event_channel.send(P2PEvent::PeerDisconnected(peer_id)) {
			Ok(_) => (),
			Err(err) => error!("Error sending P2PEvent::PeerDisconnected: {}", err),
		}
	}

	fn peer_pairing_request(
		&self,
		_nm: &NetworkManager<Self>,
		peer_id: &PeerId,
		peer_metadata: &PeerMetadata,
		extra_data: &HashMap<String, String>,
		password_resp: oneshot::Sender<Result<String, ()>>,
	) {
		self.pairing_requests
			.lock()
			.unwrap()
			.insert(peer_id.clone(), password_resp);
		match self.event_channel.send(P2PEvent::PeerPairingRequest {
			peer_id: peer_id.clone(),
			library_id: Uuid::from_str(&extra_data.get(LIBRARY_ID_EXTRA_DATA_KEY).unwrap())
				.unwrap(),
			peer_metadata: peer_metadata.clone(),
		}) {
			Ok(_) => (),
			Err(err) => error!("Error sending P2PEvent::PeerPairingRequest: {}", err),
		}
	}

	fn peer_paired<'a>(
		&'a self,
		_nm: &'a NetworkManager<Self>,
		direction: PairingParticipantType,
		peer_id: &'a PeerId,
		peer_metadata: &'a PeerMetadata,
		extra_data: &'a HashMap<String, String>,
	) -> Pin<Box<dyn Future<Output = Result<(), ()>> + Send + 'a>> {
		// TODO: Checking is peer is the same or newer version of application and hence that it's safe to join

		Box::pin(async move {
			let library_id = extra_data.get(LIBRARY_ID_EXTRA_DATA_KEY).unwrap();

			match direction {
				PairingParticipantType::Initiator => {}
				PairingParticipantType::Accepter => {
					let library_config: LibraryConfig = serde_json::from_str(
						extra_data.get(LIBRARY_CONFIG_EXTRA_DATA_KEY).unwrap(),
					)
					.unwrap();

					self.library_manager
						.create_with_id(Uuid::parse_str(library_id).unwrap(), library_config)
						.await
						.unwrap();

					// TODO: Add remote client into library database
				}
			}

			let ctx = self
				.library_manager
				.get_ctx(library_id.clone())
				.await
				.unwrap();
			ctx.db
				.node()
				.upsert(
					node::pub_id::equals(peer_id.to_string()),
					(
						node::pub_id::set(peer_id.to_string()),
						node::name::set(peer_metadata.name.clone()),
						vec![node::platform::set(0 as i32)], // TODO: Set platform correctly
					),
					vec![node::name::set(peer_metadata.name.clone())],
				)
				.exec()
				.await
				.unwrap();

			match self.event_channel.send(P2PEvent::PeerPairingComplete {
				peer_id: peer_id.clone(),
				peer_metadata: peer_metadata.clone(),
				library_id: Uuid::from_str(library_id).unwrap(), // TODO: Do this at start of function and throw if invalid
			}) {
				Ok(_) => Ok(()),
				Err(err) => {
					error!("Error sending P2PEvent::PeerPairingComplete: {}", err);
					Err(())
				}
			}
		})
	}

	fn peer_paired_rollback<'a>(
		&'a self,
		_nm: &'a NetworkManager<Self>,
		_direction: PairingParticipantType,
		_peer_id: &'a PeerId,
		_peer_metadata: &'a PeerMetadata,
		_extra_data: &'a HashMap<String, String>,
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
			let req: P2PRequest = read_value(&mut rx).await.unwrap();

			match req {
				P2PRequest::Ping => {
					println!("Received ping from '{}'", peer.id);
					write_value(&mut tx, &P2PResponse::Pong).await.unwrap();
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
) -> Result<P2PData, NetworkManagerError> {
	let identity = Identity::new().unwrap(); // TODO: Save and load from Spacedrive config
	let event_channel = mpsc::unbounded_channel();
	let pairing_requests = Arc::new(Mutex::new(HashMap::new()));
	let nm = NetworkManager::new(
		identity,
		SdP2PManager {
			library_manager,
			config,
			event_channel: event_channel.0,
			pairing_requests: pairing_requests.clone(),
		},
		NetworkManagerConfig {
			known_peers: Default::default(), // TODO: Load these from the database on startup
			listen_port: None,
			spacetunnel_url: Some(SPACETUNNEL_URL.into()),
		},
	)
	.await?;
	info!(
		"Peer '{}' listening on: {:?}",
		nm.peer_id(),
		nm.listen_addr()
	);

	// TODO: abstraction for this
	// let (mut tx, mut rx) = nm.stream(peer_id).await.unwrap();
	// tx.write_all(rmp_serde::encode::to_vec_named(&P2PRequest::Ping))
	// 	.await
	// 	.unwrap();

	Ok(P2PData {
		nm,
		event_receiver: event_channel.1,
		pairing_requests,
	})
}

pub async fn pair(
	nm: &Arc<NetworkManager<SdP2PManager>>,
	ctx: LibraryContext,
	peer_id: PeerId,
) -> CoreResponse {
	match nm
		.initiate_pairing_with_peer(
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
		.await
	{
		Ok(preshared_key) => {
			ctx.emit(CoreEvent::InvalidateQuery(ClientQuery::DiscoveredPeers))
				.await;
			ctx.emit(CoreEvent::InvalidateQuery(ClientQuery::ConnectedPeers))
				.await;

			CoreResponse::PairNode { preshared_key }
		}
		Err(err) => {
			println!("Error pairing: {:?}", err);
			CoreResponse::Null
		}
	}
}
