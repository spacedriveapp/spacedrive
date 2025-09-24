use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;
use crate::infra::job::types::ActionContextInfo;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct JobListItem {
	pub id: Uuid,
	pub name: String,
	pub status: crate::infra::job::types::JobStatus,
	pub progress: f32,
	pub action_type: Option<String>,
	pub action_context: Option<ActionContextInfo>,
}


#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct JobListOutput {
	pub jobs: Vec<JobListItem>,
}
