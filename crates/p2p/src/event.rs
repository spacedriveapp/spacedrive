use std::{net::SocketAddr, sync::Arc};

use crate::{
	spacetime::{BroadcastStream, UnicastStream},
	ConnectedPeer, DiscoveredPeer, Manager, Metadata,
};

use super::PeerId;

/// represents an event coming from the network manager.
/// This is useful for updating your UI when stuff changes on the backend.
/// You can also interact with some events to cause an event.
#[derive(Debug)]
pub enum Event<TMetadata: Metadata> {
	/// add a network interface on this node to listen for
	AddListenAddr(SocketAddr),
	/// remove a network interface from this node so that we don't listen to it
	RemoveListenAddr(SocketAddr),
	/// discovered peer on your local network
	PeerDiscovered(DiscoveredPeer<TMetadata>),
	/// a discovered peer has disappeared from the network
	PeerExpired {
		id: PeerId,
		// Will be none if we receive the expire event without having ever seen a discover event.
		metadata: Option<TMetadata>,
	},
	/// communication was established with a peer.
	/// Theere could actually be multiple connections under the hood but we smooth it over in this API.
	PeerConnected(ConnectedPeer),
	/// communication was lost with a peer.
	PeerDisconnected(PeerId),
	/// the peer has opened a new unicast substream
	PeerMessage(PeerMessageEvent<TMetadata, UnicastStream>),
	/// the peer has opened a new brodcast substream
	PeerBroadcast(PeerMessageEvent<TMetadata, BroadcastStream>),
	/// the node is shutting down
	Shutdown,
}

#[derive(Debug)]
pub struct PeerMessageEvent<TMetadata: Metadata, S> {
	pub stream_id: u64,
	pub peer_id: PeerId,
	pub manager: Arc<Manager<TMetadata>>,
	pub stream: S,
	// Prevent manual creation by end-user
	pub(crate) _priv: (),
}

impl<TMetadata: Metadata> From<PeerMessageEvent<TMetadata, UnicastStream>> for Event<TMetadata> {
	fn from(event: PeerMessageEvent<TMetadata, UnicastStream>) -> Self {
		Self::PeerMessage(event)
	}
}

impl<TMetadata: Metadata> From<PeerMessageEvent<TMetadata, BroadcastStream>> for Event<TMetadata> {
	fn from(event: PeerMessageEvent<TMetadata, BroadcastStream>) -> Self {
		Self::PeerBroadcast(event)
	}
}
