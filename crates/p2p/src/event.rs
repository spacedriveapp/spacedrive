use std::{net::SocketAddr, sync::Arc};

use crate::{spacetime::UnicastStream, spacetunnel::RemoteIdentity, ConnectedPeer, Manager};

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
	PeerDisconnected(RemoteIdentity),
	/// the peer has opened a new unicast substream
	PeerMessage(PeerMessageEvent),
	/// the node is shutting down
	Shutdown,
}

#[derive(Debug)]
pub struct PeerMessageEvent {
	pub stream_id: u64,
	pub identity: RemoteIdentity,
	pub manager: Arc<Manager>,
	pub stream: UnicastStream,
	// Prevent manual creation by end-user
	pub(crate) _priv: (),
}

impl From<PeerMessageEvent> for Event {
	fn from(event: PeerMessageEvent) -> Self {
		Self::PeerMessage(event)
	}
}
