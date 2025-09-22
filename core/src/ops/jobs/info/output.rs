
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobInfoOutput {
	pub id: Uuid,
	pub name: String,
	pub status: crate::infra::job::types::JobStatus,
	pub progress: f32,
	pub started_at: chrono::DateTime<chrono::Utc>,
	pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
	pub error_message: Option<String>,
}
