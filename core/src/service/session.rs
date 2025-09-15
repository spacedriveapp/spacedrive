use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Generic per-session state (client-agnostic)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionState {
	pub current_library_id: Option<Uuid>,
}

pub struct SessionStateService {
	state: Arc<RwLock<SessionState>>,
	data_dir: PathBuf,
}

impl SessionStateService {
	pub fn new(data_dir: PathBuf) -> Self {
		let state = Self::load(&data_dir).unwrap_or_default();
		Self { state: Arc::new(RwLock::new(state)), data_dir }
	}

	pub async fn get(&self) -> SessionState { self.state.read().await.clone() }

	pub async fn set_current_library(&self, id: Option<Uuid>) -> Result<(), Box<dyn std::error::Error>> {
		{
			let mut s = self.state.write().await;
			s.current_library_id = id;
		}
		self.save().await
	}

	async fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
		let state_file = self.data_dir.join("session_state.json");
		if let Some(parent) = state_file.parent() { std::fs::create_dir_all(parent)?; }
		let content = serde_json::to_string_pretty(&*self.state.read().await)?;
		std::fs::write(&state_file, content)?;
		Ok(())
	}

	fn load(data_dir: &Path) -> Result<SessionState, Box<dyn std::error::Error>> {
		let state_file = data_dir.join("session_state.json");
		if state_file.exists() {
			let content = std::fs::read_to_string(&state_file)?;
			Ok(serde_json::from_str(&content)?)
		} else {
			Ok(SessionState::default())
		}
	}
}


