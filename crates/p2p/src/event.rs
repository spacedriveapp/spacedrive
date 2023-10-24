use std::{net::SocketAddr, sync::Arc};

use crate::{
	spacetime::{BroadcastStream, UnicastStream},
	ConnectedPeer, Manager,
};

use super::PeerId;

/// represents an event coming from the network manager.
/// This is useful for updating your UI when stuff changes on the backend.
/// You can also interact with some events to cause an event.
#[derive(Debug)]
pub enum Event {
	/// add a network interface on this node to listen for
	AddListenAddr(SocketAddr),
	/// remove a network interface from this node so that we don't listen to it
	RemoveListenAddr(SocketAddr),
	/// communication was established with a peer.
	/// Theere could actually be multiple connections under the hood but we smooth it over in this API.
	PeerConnected(ConnectedPeer),
	/// communication was lost with a peer.
	PeerDisconnected(PeerId),
	/// the peer has opened a new unicast substream
	PeerMessage(PeerMessageEvent<UnicastStream>),
	/// the peer has opened a new brodcast substream
	PeerBroadcast(PeerMessageEvent<BroadcastStream>),
	/// the node is shutting down
	Shutdown,
}

#[derive(Debug)]
pub struct PeerMessageEvent<S> {
	pub stream_id: u64,
	pub peer_id: PeerId,
	pub manager: Arc<Manager>,
	pub stream: S,
	// Prevent manual creation by end-user
	pub(crate) _priv: (),
}

impl From<PeerMessageEvent<UnicastStream>> for Event {
	fn from(event: PeerMessageEvent<UnicastStream>) -> Self {
		Self::PeerMessage(event)
	}
}

impl From<PeerMessageEvent<BroadcastStream>> for Event {
	fn from(event: PeerMessageEvent<BroadcastStream>) -> Self {
		Self::PeerBroadcast(event)
	}
}
