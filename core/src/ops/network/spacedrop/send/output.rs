use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpacedropSendOutput {
	pub job_id: Option<Uuid>,
	pub session_id: Option<Uuid>,
}

