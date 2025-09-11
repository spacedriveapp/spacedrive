//! Output for networking init

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInitOutput {
	pub device_id: Uuid,
	pub node_id: Option<String>,
}

