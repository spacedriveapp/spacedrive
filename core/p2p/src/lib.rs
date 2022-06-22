mod discovery;
mod error;
mod identity;
mod network_manager;
mod network_manager_state;
mod p2p_application;
mod peer;
mod quic;

pub(crate) use discovery::*;
pub use error::*;
pub use identity::*;
pub use network_manager::*;
pub use network_manager_state::*;
pub use p2p_application::*;
pub use peer::*;
pub use sd_tunnel_utils::PeerId;

use dashmap::DashSet;
use quinn::{ClientConfig, Endpoint, RecvStream, SendStream, VarInt};
use std::{collections::HashMap, net::Ipv4Addr, sync::Arc};

/// NetworkManagerEvent is an event that is sent to the application which is embedding 'sd-p2p'. It allows the application to react to events that occur in the networking layer.
#[derive(Debug)]
pub enum NetworkManagerEvent {
	/// PeerDiscovered is sent when a new peer is discovered which is available be be paired with. It is recommended when this event comes in that you establish a connection with the peer if it is known.
	PeerDiscovered { peer: PeerCandidate },
	/// ConnectionEstablished is sent when a connection is established with a peer.
	ConnectionEstablished { peer: Peer },
	/// AcceptStream is sent when a networking stream is accepted by the server. The stream can be handled by the user or closed.
	AcceptStream {
		peer: Peer,
		stream: (SendStream, RecvStream),
	},
	/// TODO
	PeerRequest { peer: Peer, data: Vec<u8> },
	/// ConnectionClosed is sent when a connection is closed with a peer.
	ConnectionClosed { peer: Peer },
}

/// PeerCandidate represents a peer that has been discovered but not paired with.
#[derive(Debug, Clone)]
pub struct PeerCandidate {
	pub id: PeerId,
	pub metadata: PeerMetadata,
	pub addresses: Vec<Ipv4Addr>,
	pub port: u16,
}

/// PeerMetadata represents public metadata about a peer. This is found through the discovery process.
#[derive(Debug, Clone)]
pub struct PeerMetadata {
	pub name: String,
	pub version: Option<String>,
}

impl PeerMetadata {
	pub fn from_hashmap(peer_id: &PeerId, hashmap: &HashMap<String, String>) -> Self {
		Self {
			name: hashmap
				.get("name")
				.map(|v| v.to_string())
				.unwrap_or(peer_id.to_string()),
			version: hashmap.get("version").map(|v| v.to_string()),
		}
	}

	pub fn to_hashmap(self) -> HashMap<String, String> {
		let mut hashmap = HashMap::new();
		hashmap.insert("name".to_string(), self.name);
		if let Some(version) = self.version {
			hashmap.insert("version".to_string(), version);
		}
		hashmap
	}
}
