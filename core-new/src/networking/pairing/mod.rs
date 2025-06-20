//! Device pairing protocol implementation
//!
//! Implements the complete Spacedrive device pairing protocol with:
//! - BIP39 word-based pairing codes
//! - mDNS discovery
//! - Challenge-response authentication
//! - Session key establishment
//! - Secure device information exchange

pub mod code;
pub mod discovery;
pub mod connection;
pub mod protocol;
pub mod ui;

#[cfg(test)]
mod tests;

pub use code::*;
pub use discovery::*;
pub use connection::*;
pub use protocol::*;
pub use ui::*;

use crate::networking::{Result, NetworkError};
use uuid::Uuid;

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

/// Complete pairing orchestrator
pub struct PairingManager {
    /// Current pairing state
    state: PairingState,
    /// Active pairing session (if any)
    session: Option<PairingSession>,
}

impl PairingManager {
    pub fn new() -> Self {
        Self {
            state: PairingState::Idle,
            session: None,
        }
    }

    pub fn state(&self) -> &PairingState {
        &self.state
    }

    pub fn session(&self) -> Option<&PairingSession> {
        self.session.as_ref()
    }
}

/// A complete pairing session
pub struct PairingSession {
    /// Session ID for tracking
    pub id: Uuid,
    /// Pairing code for this session
    pub code: Option<PairingCode>,
    /// Discovery service
    pub discovery: Option<PairingDiscovery>,
    /// Secure connection
    pub connection: Option<PairingConnection>,
    /// Whether we initiated the pairing
    pub is_initiator: bool,
}

impl PairingSession {
    pub fn new(is_initiator: bool) -> Self {
        Self {
            id: Uuid::new_v4(),
            code: None,
            discovery: None,
            connection: None,
            is_initiator,
        }
    }
}