//! Input types for library sync setup operations

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

/// Action to take when setting up library sync
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum LibrarySyncAction {
	/// Share local library to remote device (creates same library with same UUID on remote)
	/// This is the primary way to create a shared library
	#[serde(rename_all = "camelCase")]
	ShareLocalLibrary { library_name: String },

	/// Join an existing remote library (creates same library with same UUID locally)
	/// Use this when the other device has already shared their library
	#[serde(rename_all = "camelCase")]
	JoinRemoteLibrary {
		remote_library_id: Uuid,
		remote_library_name: String,
	},

	/// Future: Merge two different libraries into one (combines data from both)
	/// Not yet implemented - requires full sync system
	#[serde(rename_all = "camelCase")]
	MergeLibraries {
		local_library_id: Uuid,
		remote_library_id: Uuid,
		merged_name: String,
	},
}

/// Input for setting up library sync between paired devices
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LibrarySyncSetupInput {
	/// Local device ID (should be current device)
	pub local_device_id: Uuid,

	/// Remote paired device ID
	pub remote_device_id: Uuid,

	/// Local library to set up sync for
	pub local_library_id: Uuid,

	/// Remote library to sync with (optional for RegisterOnly)
	pub remote_library_id: Option<Uuid>,

	/// Sync action to perform
	pub action: LibrarySyncAction,

	/// DEPRICATED: Which device should be the sync leader (for future sync implementation)
	pub leader_device_id: Uuid,
}
