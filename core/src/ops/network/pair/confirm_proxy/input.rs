use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct PairConfirmProxyInput {
	pub session_id: Uuid,
	pub accepted: bool,
}
