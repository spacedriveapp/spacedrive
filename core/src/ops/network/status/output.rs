//! Output types for network status

use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct NetworkStatus {
	pub running: bool,
	pub node_id: Option<String>,
	pub addresses: Vec<String>,
	pub paired_devices: usize,
	pub connected_devices: usize,
	pub version: String,
	pub relay_url: Option<String>,
}
