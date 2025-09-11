use crate::domain::addressing::SdPath;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpacedropSendInput {
	pub device_id: Uuid,
	pub paths: Vec<SdPath>,
	pub sender: Option<String>,
}

