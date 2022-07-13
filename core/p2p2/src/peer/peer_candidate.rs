use std::net::Ipv4Addr;

use sd_tunnel_utils::PeerId;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::PeerMetadata;

/// Represents a peer that has been discovered but not paired with.
/// It is called a candidate as it contains all of the information required to connection and pair with the peer.
/// A peer candidate discovered through mDNS may have been modified by an attacker on your local network but this is deemed acceptable as the attacker can only modify primitive metadata such a name or Spacedrive version which is used for pairing.
/// When we initiated communication with the device we will ensure we are talking to the correct device using PAKE (specially SPAKE2) for pairing and verifying the TLS certificate for general communication.
#[derive(Debug, Clone)]
pub struct PeerCandidate {
	pub id: PeerId,
	pub metadata: PeerMetadata,
	pub addresses: Vec<Ipv4Addr>,
	pub port: u16,
}

/// This struct exists due to `ts_rs` not supporting `Ipv4Addr`. Issue: <https://github.com/Aleph-Alpha/ts-rs/issues/110>
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
