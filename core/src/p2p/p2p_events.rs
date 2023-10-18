use sd_p2p::PeerId;
use serde::Serialize;
use specta::Type;
use uuid::Uuid;

use super::{OperatingSystem, PairingStatus, PeerMetadata};

// TODO: Split into another file
/// TODO: P2P event for the frontend
#[derive(Debug, Clone, Serialize, Type)]
#[serde(tag = "type")]
pub enum P2PEvent {
	DiscoveredPeer {
		peer_id: PeerId,
		metadata: PeerMetadata,
	},
	ExpiredPeer {
		peer_id: PeerId,
	},
	ConnectedPeer {
		peer_id: PeerId,
	},
	DisconnectedPeer {
		peer_id: PeerId,
	},
	SpacedropRequest {
		id: Uuid,
		peer_id: PeerId,
		peer_name: String,
		files: Vec<String>,
	},
	SpacedropProgress {
		id: Uuid,
		percent: u8,
	},
	SpacedropTimedout {
		id: Uuid,
	},
	SpacedropRejected {
		id: Uuid,
	},
	// Pairing was reuqest has come in.
	// This will fire on the responder only.
	PairingRequest {
		id: u16,
		name: String,
		os: OperatingSystem,
	},
	PairingProgress {
		id: u16,
		status: PairingStatus,
	}, // TODO: Expire peer + connection/disconnect
}
