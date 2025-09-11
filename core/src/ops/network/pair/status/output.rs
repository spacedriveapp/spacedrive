use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PairingStateLite {
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
	Challenge,
	ResponsePending,
	ResponseSent,
	Completed,
	Failed { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingSessionSummary {
	pub id: Uuid,
	pub state: PairingStateLite,
	pub remote_device_id: Option<Uuid>,
	pub expires_at: Option<DateTime<Utc>>, // optional if available
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairStatusOutput {
	pub sessions: Vec<PairingSessionSummary>,
}

