use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct PairVouchOutput {
	pub success: bool,
	pub pending_count: u32,
}
