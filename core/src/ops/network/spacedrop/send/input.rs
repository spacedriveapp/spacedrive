use crate::domain::addressing::SdPath;
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SpacedropSendInput {
	pub device_id: Uuid,
	pub paths: Vec<SdPath>,
	pub sender: Option<String>,
}
