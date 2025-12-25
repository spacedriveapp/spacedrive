use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

/// Input for confirming or rejecting a pairing request
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct PairConfirmInput {
	/// The session ID of the pairing request to confirm
	pub session_id: Uuid,
	/// Whether to accept (true) or reject (false) the pairing request
	pub accepted: bool,
}
