use serde::{Deserialize, Serialize};
use specta::Type;

/// Output from confirming a pairing request
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct PairConfirmOutput {
	/// Whether the confirmation was successful
	pub success: bool,
	/// Error message if confirmation failed
	pub error: Option<String>,
}
