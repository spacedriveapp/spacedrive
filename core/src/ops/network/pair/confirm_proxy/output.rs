use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct PairConfirmProxyOutput {
	pub success: bool,
	pub error: Option<String>,
}
