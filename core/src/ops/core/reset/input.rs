use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ResetDataInput {
	/// Confirmation flag to prevent accidental data loss
	pub confirm: bool,
}
