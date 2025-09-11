use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::service::network::PairingState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingSessionSummary {
	pub id: Uuid,
	pub state: PairingState,
	pub remote_device_id: Option<Uuid>,
	pub expires_at: Option<DateTime<Utc>>, // optional if available
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairStatusOutput {
	pub sessions: Vec<PairingSessionSummary>,
}

