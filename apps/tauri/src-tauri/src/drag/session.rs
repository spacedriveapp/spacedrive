use super::*;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DragSession {
	pub id: String,
	pub config: DragConfig,
	pub source_window: String,
	pub started_at: u64,
}

impl DragSession {
	pub fn new(config: DragConfig, source_window: String) -> Self {
		Self {
			id: Uuid::new_v4().to_string(),
			config,
			source_window,
			started_at: std::time::SystemTime::now()
				.duration_since(std::time::UNIX_EPOCH)
				.unwrap()
				.as_millis() as u64,
		}
	}

	pub fn to_event(&self) -> DragBeganEvent {
		DragBeganEvent {
			session_id: self.id.clone(),
			source_window: self.source_window.clone(),
			items: self.config.items.clone(),
		}
	}
}
