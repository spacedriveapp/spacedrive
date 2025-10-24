//! Library-related messages for sync setup and discovery

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Messages related to library operations between devices
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LibraryMessage {
	/// Request to discover libraries on a remote device
	DiscoveryRequest { request_id: Uuid },

	/// Response with list of libraries
	DiscoveryResponse {
		request_id: Uuid,
		libraries: Vec<LibraryDiscoveryInfo>,
	},

	/// Request to register a device in a library
	RegisterDeviceRequest {
		request_id: Uuid,
		library_id: Option<Uuid>, // None means register in all libraries
		device_id: Uuid,
		device_name: String,
		device_slug: String,
		os_name: String,
		os_version: Option<String>,
		hardware_model: Option<String>,
	},

	/// Response to device registration
	RegisterDeviceResponse {
		request_id: Uuid,
		success: bool,
		message: Option<String>,
	},

	/// Request to create a shared library on remote device
	CreateSharedLibraryRequest {
		request_id: Uuid,
		library_id: Uuid,
		library_name: String,
		description: Option<String>,
		/// Info about the requesting device (to pre-register before library creation)
		requesting_device_id: Uuid,
		requesting_device_name: String,
		requesting_device_slug: String,
	},

	/// Response to library creation request
	CreateSharedLibraryResponse {
		request_id: Uuid,
		success: bool,
		message: Option<String>,
		/// The slug this device is using in the shared library (may be renamed due to collision)
		device_slug: Option<String>,
	},

	/// Request library state for collision detection before joining
	LibraryStateRequest {
		request_id: Uuid,
		library_id: Uuid,
	},

	/// Response with library state
	LibraryStateResponse {
		request_id: Uuid,
		library_id: Uuid,
		library_name: String,
		device_slugs: Vec<String>,
		device_count: usize,
	},
}

/// Information about a library for discovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryDiscoveryInfo {
	pub id: Uuid,
	pub name: String,
	pub description: Option<String>,
	pub created_at: DateTime<Utc>,
	pub total_entries: u64,
	pub total_locations: u64,
	pub total_size_bytes: u64,
	pub device_count: u64,
}
