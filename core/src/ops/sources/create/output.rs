//! Source creation output

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

/// Output from creating a new archive source
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CreateSourceOutput {
	/// The ID of the newly created source
	pub id: Uuid,
	/// The display name of the source
	pub name: String,
	/// The adapter ID used
	pub adapter_id: String,
	/// Current status (usually "idle" initially)
	pub status: String,
}

impl CreateSourceOutput {
	pub fn new(id: Uuid, name: String, adapter_id: String, status: String) -> Self {
		Self {
			id,
			name,
			adapter_id,
			status,
		}
	}
}
