//! Pairing protocol message definitions

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::proxy::{AcceptedDevice, RejectedDevice};
use crate::service::network::device::{DeviceInfo, SessionKeys};

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
	Reject { session_id: Uuid, reason: String },
	// Voucher -> Other device: "Trust this new device"
	ProxyPairingRequest {
		session_id: Uuid,
		vouchee_device_info: DeviceInfo,
		vouchee_public_key: Vec<u8>,
		voucher_device_id: Uuid,
		voucher_signature: Vec<u8>,
		timestamp: chrono::DateTime<chrono::Utc>,
		proxied_session_keys: SessionKeys,
	},
	// Other device -> Voucher: "I accept or reject this vouch"
	ProxyPairingResponse {
		session_id: Uuid,
		accepting_device_id: Uuid,
		accepted: bool,
		reason: Option<String>,
	},
	// Voucher -> Vouchee: "These devices accepted you"
	ProxyPairingComplete {
		session_id: Uuid,
		voucher_device_id: Uuid,
		accepted_by: Vec<AcceptedDevice>,
		rejected_by: Vec<RejectedDevice>,
	},
}
