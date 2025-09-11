use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairGenerateOutput {
	pub code: String,
	pub session_id: Uuid,
	pub expires_at: DateTime<Utc>,
}

