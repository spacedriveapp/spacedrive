//! Output types for network status

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkStatus {
	pub running: bool,
	pub node_id: Option<String>,
	pub addresses: Vec<String>,
	pub paired_devices: usize,
	pub connected_devices: usize,
	pub version: String,
}

