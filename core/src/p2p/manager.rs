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
	read_value, write_value, NetworkManager, OperationSystem, P2PManager, PairingParticipantType,
	Peer, PeerId, PeerMetadata,
};
use tokio::sync::{mpsc, oneshot};
use tracing::error;
use uuid::Uuid;

use crate::{
	library::{LibraryConfig, LibraryManager},
	node::NodeConfigManager,
	prisma::node,
};

use super::{P2PEvent, P2PRequest, P2PResponse};

const LIBRARY_ID_EXTRA_DATA_KEY: &str = "libraryId";

const LIBRARY_CONFIG_EXTRA_DATA_KEY: &str = "libraryData";

// SdP2PManager is part of your application and allows you to hook into the behavior of the P2PManager.
#[derive(Clone)]
pub struct SdP2PManager {
	pub(super) library_manager: Arc<LibraryManager>,
	pub(super) config: Arc<NodeConfigManager>,
	/// event_channel is used to send events back to the Spacedrive main event loop
	pub(super) event_channel: mpsc::UnboundedSender<P2PEvent>,
	#[allow(clippy::type_complexity)]
	pub(super) pairing_requests: Arc<Mutex<HashMap<PeerId, oneshot::Sender<Result<String, ()>>>>>,
}

impl P2PManager for SdP2PManager {
	const APPLICATION_NAME: &'static str = "spacedrive";

	fn get_metadata(&self) -> PeerMetadata {
		PeerMetadata {
			// TODO: `block_on` needs to be removed from here!
			name: block_on(self.config.get()).name,
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
			library_id: Uuid::from_str(extra_data.get(LIBRARY_ID_EXTRA_DATA_KEY).unwrap()).unwrap(),
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
			let library_id =
				Uuid::parse_str(extra_data.get(LIBRARY_ID_EXTRA_DATA_KEY).unwrap()).unwrap();

			match direction {
				PairingParticipantType::Initiator => {}
				PairingParticipantType::Accepter => {
					let library_config: LibraryConfig = serde_json::from_str(
						extra_data.get(LIBRARY_CONFIG_EXTRA_DATA_KEY).unwrap(),
					)
					.unwrap();

					self.library_manager
						.create_with_id(library_id, library_config)
						.await
						.unwrap();

					// TODO: Add remote client into library database
				}
			}

			let ctx = self.library_manager.get_ctx(library_id).await.unwrap();
			ctx.db
				.node()
				.upsert(
					node::pub_id::equals(peer_id.as_bytes().to_vec()),
					(
						peer_id.as_bytes().to_vec(),
						peer_metadata.name.clone(),
						vec![node::platform::set(0_i32)], // TODO: Set platform correctly
					),
					vec![node::name::set(peer_metadata.name.clone())],
				)
				.exec()
				.await
				.unwrap();

			match self.event_channel.send(P2PEvent::PeerPairingComplete {
				peer_id: peer_id.clone(),
				peer_metadata: peer_metadata.clone(),
				library_id,
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
				P2PRequest::SyncMessage(msg) => {
					println!("Received sync message from '{}': {:?}", peer.id, msg);
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
