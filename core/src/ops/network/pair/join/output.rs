use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct PairJoinOutput {
	pub paired_device_id: Uuid,
	pub device_name: String,
}

