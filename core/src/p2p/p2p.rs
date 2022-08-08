use std::{
	collections::HashMap,
	sync::{Arc, Mutex},
};

use p2p::{
	Identity, NetworkManager, NetworkManagerConfig, NetworkManagerError, PeerId, PeerMetadata,
};
use tokio::sync::{mpsc, oneshot};
use tracing::info;
use uuid::Uuid;

use crate::{
	library::{LibraryContext, LibraryManager},
	node::NodeConfigManager,
};

use super::{P2PRequest, SdP2PManager};

pub const LIBRARY_ID_EXTRA_DATA_KEY: &str = "libraryId";
pub const LIBRARY_CONFIG_EXTRA_DATA_KEY: &str = "libraryData";

#[derive(Debug, Clone)]
#[allow(clippy::enum_variant_names)]
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

pub struct SdP2P {
	pub nm: Arc<NetworkManager<SdP2PManager>>,
	pub event_receiver: mpsc::UnboundedReceiver<P2PEvent>,
	#[allow(clippy::type_complexity)]
	pub pairing_requests: Arc<Mutex<HashMap<PeerId, oneshot::Sender<Result<String, ()>>>>>,
}

impl SdP2P {
	pub async fn init(
		library_manager: Arc<LibraryManager>,
		config: Arc<NodeConfigManager>,
	) -> Result<SdP2P, NetworkManagerError> {
		let identity = {
			let config = config.get().await;
			Identity::from_raw(config.p2p_cert, config.p2p_key)?
		};
		let spacetunnel_url = Some(
			config
				.get()
				.await
				.spacetunnel_addr
				.unwrap_or_else(|| "tunnel.spacedrive.com:9000".into()),
		);
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
				spacetunnel_url,
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

		Ok(SdP2P {
			nm,
			event_receiver: event_channel.1,
			pairing_requests,
		})
	}

	// TODO: Error type
	pub async fn pair(&self, ctx: &LibraryContext, peer_id: PeerId) -> Result<String, ()> {
		self.nm
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
			.map_err(|err| {
				println!("Error pairing: {:?}", err);
			})
	}

	pub fn broadcast(&self, msg: P2PRequest) -> Result<(), rmp_serde::encode::Error> {
		self.nm.broadcast(rmp_serde::encode::to_vec_named(&msg)?);
		Ok(())
	}
}
