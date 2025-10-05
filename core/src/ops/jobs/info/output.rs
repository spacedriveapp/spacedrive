use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct JobInfoOutput {
	pub id: Uuid,
	pub name: String,
	pub status: crate::infra::job::types::JobStatus,
	pub progress: f32,
	pub started_at: chrono::DateTime<chrono::Utc>,
	pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
	pub error_message: Option<String>,
}
