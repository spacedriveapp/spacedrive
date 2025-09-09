use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreStatus {
	pub version: String,
	pub library_count: usize,
}


