use std::{collections::HashMap, future::Future, pin::Pin};

use quinn::{RecvStream, SendStream};
use sd_tunnel_utils::PeerId;
use tokio::sync::oneshot;

use crate::{NetworkManager, Peer, PeerMetadata};

/// TODO: I despise the name of this enum but couldn't thing of anything better
pub enum PairingDirection {
	// This device initiated the pairing request
	Initiator,
	// This device accepted a pairing request from another device
	Accepter,
}

/// TODO
pub trait P2PManager: Clone + Send + Sync + Sized + 'static {
	const APPLICATION_NAME: &'static str;

	/// is called to get the metadata of the application. This metadata is sent as part of the discovery payload.
	fn get_metadata(&self) -> PeerMetadata;

	/// TODO
	fn peer_discovered(&self, nm: &NetworkManager<Self>, peer_id: &PeerId) {}

	/// TODO
	fn peer_expired(&self, nm: &NetworkManager<Self>, peer_id: PeerId) {}

	/// TODO
	fn peer_connected(&self, nm: &NetworkManager<Self>, peer_id: PeerId) {}

	/// TODO
	fn peer_disconnected(&self, nm: &NetworkManager<Self>, peer_id: PeerId) {}

	/// TODO: When a peer has requested to pair with you.
	fn peer_pairing_request(
		&self,
		nm: &NetworkManager<Self>,
		peer_id: &PeerId,
		metadata: &PeerMetadata,
		extra_data: &HashMap<String, String>,
		password_resp: oneshot::Sender<Result<String, ()>>,
	) {
	}

	/// TODO
	/// TODO: Error type
	fn peer_paired<'a>(
		&'a self,
		nm: &'a NetworkManager<Self>,
		direction: PairingDirection,
		peer_id: &'a PeerId,
		peer_metadata: &'a PeerMetadata,
		extra_data: &'a HashMap<String, String>,
	) -> Pin<Box<dyn Future<Output = Result<(), ()>> + Send + 'a>>;

	/// TODO
	fn peer_paired_rollback<'a>(
		&'a self,
		nm: &'a NetworkManager<Self>,
		direction: PairingDirection,
		peer_id: &'a PeerId,
		peer_metadata: &'a PeerMetadata,
		extra_data: &'a HashMap<String, String>,
	) -> Pin<Box<dyn Future<Output = ()> + Send + Sync + 'a>>;

	/// TODO
	fn accept_stream(&self, peer: &Peer<Self>, stream: (SendStream, RecvStream)) {}
}
