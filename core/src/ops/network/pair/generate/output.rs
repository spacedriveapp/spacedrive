use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct PairGenerateOutput {
	pub code: String,
	pub session_id: Uuid,
	pub expires_at: DateTime<Utc>,
	/// QR code JSON format (includes NodeId and relay URL for remote pairing)
	pub qr_json: String,
}
