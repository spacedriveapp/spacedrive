//! Output types for library sync setup operations

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

/// Result of library sync setup operation
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LibrarySyncSetupOutput {
	/// Whether setup was successful
	pub success: bool,

	/// Local library ID that was configured
	pub local_library_id: Uuid,

	/// Remote library ID that was linked (if applicable)
	pub remote_library_id: Option<Uuid>,

	/// Whether devices were successfully registered in each other's libraries
	pub devices_registered: bool,

	/// Message describing the result
	pub message: String,
}
