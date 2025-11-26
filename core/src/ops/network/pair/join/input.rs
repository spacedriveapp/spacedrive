use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct PairJoinInput {
	pub code: String,
	/// Optional node ID for relay-based pairing (enables cross-network connections)
	pub node_id: Option<String>,
}
