use crate::service::network::PairingState;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub enum SerializablePairingState {
	Idle,
	GeneratingCode,
	Broadcasting,
	Scanning,
	WaitingForConnection,
	Connecting,
	Authenticating,
	ExchangingKeys,
	/// Awaiting user confirmation with 2-digit verification code
	AwaitingUserConfirmation {
		confirmation_code: String,
		expires_at: DateTime<Utc>,
	},
	/// Legacy state - kept for backward compatibility
	AwaitingConfirmation,
	EstablishingSession,
	ChallengeReceived,
	ResponsePending,
	ResponseSent,
	Completed,
	Failed { reason: String },
	/// Pairing was rejected by the user
	Rejected { reason: String },
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
			PairingState::AwaitingUserConfirmation {
				confirmation_code,
				expires_at,
			} => Self::AwaitingUserConfirmation {
				confirmation_code,
				expires_at,
			},
			PairingState::AwaitingConfirmation => Self::AwaitingConfirmation,
			PairingState::EstablishingSession => Self::EstablishingSession,
			PairingState::ChallengeReceived { .. } => Self::ChallengeReceived,
			PairingState::ResponsePending { .. } => Self::ResponsePending,
			PairingState::ResponseSent => Self::ResponseSent,
			PairingState::Completed => Self::Completed,
			PairingState::Failed { reason } => Self::Failed { reason },
			PairingState::Rejected { reason } => Self::Rejected { reason },
		}
	}
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct PairingSessionSummary {
	pub id: Uuid,
	pub state: SerializablePairingState,
	pub remote_device_id: Option<Uuid>,
	pub remote_device_name: Option<String>,
	pub remote_device_os: Option<String>,
	pub expires_at: Option<DateTime<Utc>>,
	/// Confirmation code if awaiting user confirmation (initiator only)
	pub confirmation_code: Option<String>,
	/// When the confirmation expires (initiator only)
	pub confirmation_expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct PairStatusOutput {
	pub sessions: Vec<PairingSessionSummary>,
}
