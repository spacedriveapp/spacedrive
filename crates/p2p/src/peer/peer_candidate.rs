use std::net::Ipv4Addr;

use serde::{Deserialize, Serialize};
use specta::Type;
use tunnel_utils::PeerId;

use crate::PeerMetadata;

/// Represents a peer that has been discovered but not paired with.
/// It is called a candidate as it contains all of the information required to connection and pair with the peer.
/// A peer candidate discovered through mDNS may have been modified by an attacker on your local network but this is deemed acceptable as the attacker can only modify primitive metadata such a name or Spacedrive version which is used for pairing.
/// When we initiated communication with the device we will ensure we are talking to the correct device using PAKE (specially SPAKE2) for pairing and verifying the TLS certificate for general communication.
#[derive(Debug, Clone, Type, Serialize, Deserialize)]
pub struct PeerCandidate {
	pub id: PeerId,
	pub metadata: PeerMetadata,
	pub addresses: Vec<Ipv4Addr>,
	pub port: u16,
}
