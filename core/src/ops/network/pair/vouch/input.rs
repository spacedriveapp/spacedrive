use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct PairVouchInput {
	pub session_id: Uuid,
	pub target_device_ids: Vec<Uuid>,
}
