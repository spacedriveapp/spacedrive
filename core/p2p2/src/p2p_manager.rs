use std::{collections::HashMap, future::Future, pin::Pin};

use quinn::{RecvStream, SendStream};
use sd_tunnel_utils::PeerId;
use tokio::sync::oneshot;

use crate::{NetworkManager, Peer, PeerMetadata};

/// Represents the type of the peer participating in pairing. This is useful for the P2PManager application to know but is not used in the P2PManager itself.
pub enum PairingParticipantType {
	// This peer initiated the pairing request
	Initiator,
	// This peer accepted a pairing request initiated by another device
	Accepter,
}

/// Is implement by the application which is embedding this P2P library.
/// This trait allows your application which holds the users state to hook into the P2P lifecycle and make decisions from the state it holds.
pub trait P2PManager: Clone + Send + Sync + Sized + 'static {
	const APPLICATION_NAME: &'static str;

	/// Called to get the metadata of the application. This metadata is sent as part of the discovery payload.
	fn get_metadata(&self) -> PeerMetadata;

	/// Called when a peer is discovered using any of the available discovery mechanisms .
	fn peer_discovered(&self, nm: &NetworkManager<Self>, peer_id: &PeerId) {}

	/// Called when a peer that had previously been discovered is now unavailable.
	/// This could be due to the peer announcing it is going offline or due to a timeout.
	fn peer_expired(&self, nm: &NetworkManager<Self>, peer_id: PeerId) {}

	/// Called when a connection is established with a peer.
	/// This will happen after pairing or if a peer that is in the [NetworkManager]'s `known_peers` list is discovered.
	fn peer_connected(&self, nm: &NetworkManager<Self>, peer_id: PeerId) {}

	/// Called when a connection to a peer is disconnected.
	/// This could occur due to the remote peer announcing it is going offline, or the device not responding to network activity for a certain timeout.
	fn peer_disconnected(&self, nm: &NetworkManager<Self>, peer_id: PeerId) {}

	/// Called when a peer request to pair with you. The application should accept or reject the pairing request by returning the preshared_key enter by the user through the `password_resp` oneshot channel.
	/// The application MUST respond to the channel regardless of result.
	fn peer_pairing_request(
		&self,
		nm: &NetworkManager<Self>,
		peer_id: &PeerId,
		metadata: &PeerMetadata,
		extra_data: &HashMap<String, String>,
		password_resp: oneshot::Sender<Result<String, ()>>,
	) {
	}

	/// Called when a peer has been paired with you. This function will block the pairing process until it is complete.
	/// Pairing MAY fail after this function is completed due to the nature of having to run it on both machines. It is expected any changes will be reverted in `peer_paired_rollback`.
	fn peer_paired<'a>(
		&'a self,
		nm: &'a NetworkManager<Self>,
		direction: PairingParticipantType,
		peer_id: &'a PeerId,
		peer_metadata: &'a PeerMetadata,
		extra_data: &'a HashMap<String, String>,
	) -> Pin<Box<dyn Future<Output = Result<(), ()>> + Send + 'a>>;

	/// Called when pairing failed but `peer_paired` was called. This function will undo any changes that may of been made by `peer_paired`.
	fn peer_paired_rollback<'a>(
		&'a self,
		nm: &'a NetworkManager<Self>,
		direction: PairingParticipantType,
		peer_id: &'a PeerId,
		peer_metadata: &'a PeerMetadata,
		extra_data: &'a HashMap<String, String>,
	) -> Pin<Box<dyn Future<Output = ()> + Send + Sync + 'a>>;

	/// Called when a network stream is created. This will contain your application code to communicate with the remote device.
	fn accept_stream(&self, peer: &Peer<Self>, stream: (SendStream, RecvStream)) {}
}
