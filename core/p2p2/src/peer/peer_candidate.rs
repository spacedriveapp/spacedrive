use std::net::Ipv4Addr;

use sd_tunnel_utils::PeerId;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::PeerMetadata;

/// represents a peer that has been discovered but not paired with.
#[derive(Debug, Clone)]
pub struct PeerCandidate {
	pub id: PeerId,
	pub metadata: PeerMetadata,
	pub addresses: Vec<Ipv4Addr>,
	pub port: u16,
}

/// ts_rs does not support Ipv4Addr. A PR would be a good idea!
#[derive(Debug, Clone, TS, Serialize, Deserialize)]
pub struct PeerCandidateTS {
	pub id: PeerId,
	pub metadata: PeerMetadata,
	pub addresses: Vec<String>,
	pub port: u16,
}

impl From<PeerCandidate> for PeerCandidateTS {
	fn from(pc: PeerCandidate) -> Self {
		PeerCandidateTS {
			id: pc.id,
			metadata: pc.metadata,
			addresses: pc.addresses.iter().map(|a| a.to_string()).collect(),
			port: pc.port,
		}
	}
}
