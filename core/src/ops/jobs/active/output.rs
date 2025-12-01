use crate::infra::job::types::{ActionContextInfo, JobStatus};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ActiveJobItem {
	pub id: Uuid,
	pub name: String,
	pub status: JobStatus,
	pub progress: f32,
	pub action_type: Option<String>,
	pub action_context: Option<ActionContextInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ActiveJobsOutput {
	pub jobs: Vec<ActiveJobItem>,
	pub running_count: u32,
	pub paused_count: u32,
}
