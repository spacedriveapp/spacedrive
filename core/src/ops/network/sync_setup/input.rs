//! Input types for library sync setup operations

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

/// Action to take when setting up library sync
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum LibrarySyncAction {
	/// Keep libraries separate, only register devices in each other's libraries
	/// This allows Spacedrop and future selective sync without full library merge
	RegisterOnly,

	/// Future: Merge remote library into local (local becomes leader)
	#[serde(rename_all = "camelCase")]
	MergeIntoLocal { remote_library_id: Uuid },

	/// Future: Merge local library into remote (remote becomes leader)
	#[serde(rename_all = "camelCase")]
	MergeIntoRemote { local_library_id: Uuid },

	/// Future: Create new shared library (choose leader)
	#[serde(rename_all = "camelCase")]
	CreateShared {
		leader_device_id: Uuid,
		name: String,
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

	/// Which device should be the sync leader (for future sync implementation)
	pub leader_device_id: Uuid,
}
