//! Pairing protocol message definitions

use crate::infrastructure::networking::device::DeviceInfo;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Messages exchanged during the pairing protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PairingMessage {
    // Pairing request with device info
    PairingRequest {
        session_id: Uuid,
        device_info: DeviceInfo,
        public_key: Vec<u8>,
    },
    // Pairing challenge
    Challenge {
        session_id: Uuid,
        challenge: Vec<u8>,
        device_info: DeviceInfo,  // Initiator's device info
    },
    // Pairing response with signed challenge
    Response {
        session_id: Uuid,
        response: Vec<u8>,
        device_info: DeviceInfo,
    },
    // Pairing completion
    Complete {
        session_id: Uuid,
        success: bool,
        reason: Option<String>,
    },
}