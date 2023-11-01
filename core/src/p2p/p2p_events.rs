use sd_p2p::spacetunnel::RemoteIdentity;
use serde::Serialize;
use specta::Type;
use uuid::Uuid;

use super::{OperatingSystem, PairingStatus, PeerMetadata};

/// TODO: P2P event for the frontend
#[derive(Debug, Clone, Serialize, Type)]
#[serde(tag = "type")]
pub enum P2PEvent {
	DiscoveredPeer {
		identity: RemoteIdentity,
		metadata: PeerMetadata,
	},
	ExpiredPeer {
		identity: RemoteIdentity,
	},
	ConnectedPeer {
		identity: RemoteIdentity,
	},
	DisconnectedPeer {
		identity: RemoteIdentity,
	},
	SpacedropRequest {
		id: Uuid,
		identity: RemoteIdentity,
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
