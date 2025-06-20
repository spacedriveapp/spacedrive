//! Device pairing protocol implementation
//!
//! Implements the complete Spacedrive device pairing protocol with:
//! - BIP39 word-based pairing codes
//! - mDNS discovery
//! - Challenge-response authentication
//! - Session key establishment
//! - Secure device information exchange

pub mod code;
pub mod ui;
pub mod protocol;

#[cfg(test)]
mod tests;

pub use code::*;
pub use ui::*;

use crate::networking::{Result, NetworkError, DeviceInfo};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};

/// Pairing session state
#[derive(Debug, Clone, PartialEq)]
pub enum PairingState {
    /// Initial state - waiting to start
    Idle,
    /// Generating pairing code
    GeneratingCode,
    /// Broadcasting availability for pairing
    Broadcasting,
    /// Scanning for devices to pair with
    Scanning,
    /// Establishing secure connection
    Connecting,
    /// Performing challenge-response authentication
    Authenticating,
    /// Exchanging device information and keys
    ExchangingKeys,
    /// Waiting for user confirmation
    AwaitingConfirmation,
    /// Establishing session keys
    EstablishingSession,
    /// Pairing completed successfully
    Completed,
    /// Pairing failed
    Failed(String),
}

/// Messages exchanged during pairing protocol over libp2p
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PairingMessage {
    /// Initial challenge from initiator
    Challenge {
        initiator_nonce: [u8; 16],
        timestamp: DateTime<Utc>,
    },
    /// Response to challenge from joiner
    ChallengeResponse {
        response_hash: [u8; 32],
        joiner_nonce: [u8; 16],
        timestamp: DateTime<Utc>,
    },
    /// Device information exchange
    DeviceInfo {
        device_info: DeviceInfo,
        timestamp: DateTime<Utc>,
    },
    /// Pairing accepted by user
    PairingAccepted {
        timestamp: DateTime<Utc>,
    },
    /// Pairing rejected by user
    PairingRejected {
        reason: String,
        timestamp: DateTime<Utc>,
    },
    /// Session keys establishment
    SessionKeys {
        send_key: [u8; 32],
        receive_key: [u8; 32],
        mac_key: [u8; 32],
        timestamp: DateTime<Utc>,
    },
}

/// Session keys for encrypted communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionKeys {
    /// Key for sending data
    pub send_key: [u8; 32],
    /// Key for receiving data
    pub receive_key: [u8; 32],
    /// Key for message authentication
    pub mac_key: [u8; 32],
}

/// Simple pairing manager for basic state tracking (replaced by LibP2PPairingProtocol)
pub struct PairingManager {
    /// Current pairing state
    state: PairingState,
}

impl PairingManager {
    pub fn new() -> Self {
        Self {
            state: PairingState::Idle,
        }
    }

    pub fn state(&self) -> &PairingState {
        &self.state
    }
}