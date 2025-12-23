use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DeviceRevokeInput {
	pub device_id: Uuid,

	/// Whether to also remove the device from all library databases
	///
	/// If false (default), only unpairs from network but keeps device history in libraries.
	/// If true, completely removes device from libraries (deletes all records).
	#[serde(default)]
	pub remove_from_library: bool,
}
