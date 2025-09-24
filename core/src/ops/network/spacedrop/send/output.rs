use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SpacedropSendOutput {
	pub job_id: Option<Uuid>,
	pub session_id: Option<Uuid>,
}

