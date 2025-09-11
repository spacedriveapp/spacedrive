use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairCancelOutput {
	pub cancelled: bool,
}

