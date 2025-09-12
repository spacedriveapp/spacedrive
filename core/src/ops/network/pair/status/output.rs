use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::service::network::PairingState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SerializablePairingState {
	Idle,
    GeneratingCode,
    Broadcasting,
    Scanning,
    WaitingForConnection,
    Connecting,
    Authenticating,
    ExchangingKeys,
    AwaitingConfirmation,
    EstablishingSession,
    ChallengeReceived,
    ResponsePending,
    ResponseSent,
    Completed,
    Failed { reason: String },
}

impl From<PairingState> for SerializablePairingState {
    fn from(state: PairingState) -> Self {
        match state {
            PairingState::Idle => Self::Idle,
            PairingState::GeneratingCode => Self::GeneratingCode,
            PairingState::Broadcasting => Self::Broadcasting,
            PairingState::Scanning => Self::Scanning,
            PairingState::WaitingForConnection => Self::WaitingForConnection,
            PairingState::Connecting => Self::Connecting,
            PairingState::Authenticating => Self::Authenticating,
            PairingState::ExchangingKeys => Self::ExchangingKeys,
            PairingState::AwaitingConfirmation => Self::AwaitingConfirmation,
            PairingState::EstablishingSession => Self::EstablishingSession,
            PairingState::ChallengeReceived { .. } => Self::ChallengeReceived,
            PairingState::ResponsePending { .. } => Self::ResponsePending,
            PairingState::ResponseSent => Self::ResponseSent,
            PairingState::Completed => Self::Completed,
            PairingState::Failed { reason } => Self::Failed { reason },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingSessionSummary {
	pub id: Uuid,
	pub state: SerializablePairingState,
	pub remote_device_id: Option<Uuid>,
	pub expires_at: Option<DateTime<Utc>>, // optional if available
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairStatusOutput {
	pub sessions: Vec<PairingSessionSummary>,
}

