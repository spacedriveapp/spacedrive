use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairGenerateInput {
	pub auto_accept: bool,
}

