use chrono::{DateTime, Utc};
use serde::Serialize;
use specta::Type;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Type)]
pub struct JobProgressEvent {
	pub id: Uuid,
	pub library_id: Uuid,
	pub task_count: i32,
	pub completed_task_count: i32,
	pub phase: String,
	pub message: String,
	pub info: String,
	pub estimated_completion: DateTime<Utc>,
}
