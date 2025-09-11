use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairJoinOutput {
	pub paired_device_id: Uuid,
	pub device_name: String,
}

