use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkStopOutput {
	pub stopped: bool,
}

