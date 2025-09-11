//! Initialize networking input

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInitInput {
	pub password: Option<String>,
}

