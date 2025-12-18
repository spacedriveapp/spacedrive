use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ResetDataOutput {
	/// Whether the reset was successful
	pub success: bool,
	/// Message describing the result
	pub message: String,
}
