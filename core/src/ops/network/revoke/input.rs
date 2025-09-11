use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceRevokeInput {
	pub device_id: Uuid,
}

