use std::{
	fmt::{self, Formatter},
	net::SocketAddr,
};

use libp2p::PeerId;

use crate::{spacetunnel::RemoteIdentity, Metadata};

/// Represents a discovered peer.
/// This is held by [Manager] to keep track of discovered peers
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct DiscoveredPeer<TMeta: Metadata> {
	/// the public key of the discovered peer
	pub identity: RemoteIdentity,
	/// the libp2p peer id of the discovered peer
	#[serde(skip)]
	pub peer_id: PeerId,
	/// get the metadata of the discovered peer
	pub metadata: TMeta,
	/// get the addresses of the discovered peer
	pub addresses: Vec<SocketAddr>,
}

// `Manager` impls `Debug` but it causes infinite loop and stack overflow, lmao.
impl<TMeta: Metadata> fmt::Debug for DiscoveredPeer<TMeta> {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		f.debug_struct("DiscoveredPeer")
			.field("peer_id", &self.peer_id)
			.field("metadata", &self.metadata)
			.field("addresses", &self.addresses)
			.finish()
	}
}

/// Represents a connected peer.
/// This is held by [Manager] to keep track of connected peers
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ConnectedPeer {
	/// get the identity of the discovered peer
	pub identity: RemoteIdentity,
	/// Did I open the connection?
	pub establisher: bool,
}
