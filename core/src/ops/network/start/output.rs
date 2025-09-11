use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkStartOutput {
	pub started: bool,
}

