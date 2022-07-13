use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::PeerMetadata;

/// Is sent as the first payload in each connection to establish the information and intent of the remote device.
/// This is sent by the QUIC client to the QUIC server.
#[derive(Debug, Serialize, Deserialize)]
pub enum ConnectionEstablishmentPayload {
	PairingRequest {
		pake_msg: Vec<u8>,
		metadata: PeerMetadata,
		extra_data: HashMap<String, String>,
	},
	ConnectionRequest, // TODO: Add `PeerMetadata` as argument to this.
}

/// PairingPayload are exchanged during the pairing process to establish a secure long term relationship.
#[derive(Debug, Serialize, Deserialize)]
pub enum PairingPayload {
	PairingAccepted {
		pake_msg: Vec<u8>,
		metadata: PeerMetadata,
	},
	PairingComplete,
	PairingFailed,
}
