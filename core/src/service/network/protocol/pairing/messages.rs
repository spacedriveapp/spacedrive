//! Pairing protocol message definitions

use crate::service::network::device::DeviceInfo;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Messages exchanged during the pairing protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PairingMessage {
	/// Pairing request with device info (Joiner -> Initiator)
	PairingRequest {
		session_id: Uuid,
		device_info: DeviceInfo,
		public_key: Vec<u8>,
	},
	/// Pairing challenge (Initiator -> Joiner)
	Challenge {
		session_id: Uuid,
		challenge: Vec<u8>,
		device_info: DeviceInfo, // Initiator's device info
	},
	/// Pairing response with signed challenge (Joiner -> Initiator)
	Response {
		session_id: Uuid,
		response: Vec<u8>,
		device_info: DeviceInfo,
	},
	/// Pairing completion (Initiator -> Joiner)
	Complete {
		session_id: Uuid,
		success: bool,
		reason: Option<String>,
	},
	/// Pairing rejected by user (Initiator -> Joiner)
	///
	/// Sent when the initiator's user rejects the pairing request
	/// or when the confirmation times out.
	Reject {
		session_id: Uuid,
		reason: String,
	},
}
