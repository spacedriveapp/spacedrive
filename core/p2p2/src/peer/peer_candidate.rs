use std::net::Ipv4Addr;

use sd_tunnel_utils::PeerId;

use crate::PeerMetadata;

/// represents a peer that has been discovered but not paired with.
#[derive(Debug, Clone)]
pub struct PeerCandidate {
	pub id: PeerId,
	pub metadata: PeerMetadata,
	pub addresses: Vec<Ipv4Addr>,
	pub port: u16,
}
