use crate::infra::job::types::ActionContextInfo;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct JobListItem {
	pub id: Uuid,
	pub name: String,
	pub device_id: Uuid,
	pub status: crate::infra::job::types::JobStatus,
	pub progress: f32,
	pub action_type: Option<String>,
	pub action_context: Option<ActionContextInfo>,
	pub created_at: DateTime<Utc>,
	pub started_at: Option<DateTime<Utc>>,
	pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct JobListOutput {
	pub jobs: Vec<JobListItem>,
}
