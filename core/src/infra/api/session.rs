//! Session context and authentication types
//!
//! This module defines the rich session context that gets passed to all operations,
//! replacing the simple library_id parameter with comprehensive session information.

use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;
use uuid::Uuid;

/// Rich session context passed to all operations
/// This replaces the simple library_id parameter with comprehensive session info
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SessionContext {
	/// Authentication information for this session
	pub auth: AuthenticationInfo,

	/// Currently selected library (if any)
	pub current_library_id: Option<Uuid>,

	/// User preferences and permissions for this session
	pub permissions: PermissionSet,

	/// Request metadata for audit trails and tracking
	pub request_metadata: RequestMetadata,

	/// Device context for this session
	pub device_context: DeviceContext,
}

/// Authentication information for the current session
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct AuthenticationInfo {
	/// User ID if user is authenticated (future feature)
	pub user_id: Option<Uuid>,

	/// Device making the request
	pub device_id: Uuid,

	/// Current authentication level
	pub authentication_level: AuthLevel,

	/// When this session was created
	pub session_created_at: chrono::DateTime<chrono::Utc>,

	/// Session expiry (for future user sessions)
	pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Authentication levels in order of privilege
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Type)]
pub enum AuthLevel {
	/// No authentication - limited access
	None,

	/// Device-level authentication - normal operations
	Device,

	/// User-level authentication - personal operations (future)
	User(Uuid),

	/// Admin-level authentication - system operations (future)
	Admin(Uuid),
}

// PermissionSet is defined in permissions.rs to avoid duplication
pub use super::permissions::PermissionSet;

// LibraryPermissions is also defined in permissions.rs
pub use super::permissions::LibraryPermissions;

/// Request metadata for audit trails and tracking
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct RequestMetadata {
	/// Unique ID for this request
	pub request_id: Uuid,

	/// When the request was made
	pub timestamp: chrono::DateTime<chrono::Utc>,

	/// Source of the request (CLI, Swift, etc.)
	pub source: RequestSource,

	/// IP address if applicable (for network requests)
	pub client_ip: Option<String>,

	/// User agent if applicable
	pub user_agent: Option<String>,
}

/// Source of an API request
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub enum RequestSource {
	/// CLI application
	Cli,


	/// Swift desktop application
	Swift,

	/// Internal system operation
	Internal,

	/// Unknown/other source
	Other(String),
}

/// Device context for the session
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DeviceContext {
	/// Device identifier
	pub device_id: Uuid,

	/// Device name
	pub device_name: String,

	/// Operating system
	pub os: String,

	/// Hardware model
	pub hardware_model: String,

	/// Device capabilities
	pub capabilities: DeviceCapabilities,
}

/// Device capabilities that affect operation availability
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DeviceCapabilities {
	/// Can execute long-running jobs
	pub supports_background_jobs: bool,

	/// Can access network features
	pub supports_networking: bool,

	/// Can access file system operations
	pub supports_file_operations: bool,

	/// Available storage space
	pub available_storage: Option<u64>,
}

impl SessionContext {
	/// Create a basic device session (current default)
	pub fn device_session(device_id: Uuid, device_name: String) -> Self {
		Self {
			auth: AuthenticationInfo {
				user_id: None,
				device_id,
				authentication_level: AuthLevel::Device,
				session_created_at: chrono::Utc::now(),
				expires_at: None,
			},
			current_library_id: None,
			permissions: PermissionSet::device_default(),
			request_metadata: RequestMetadata {
				request_id: Uuid::new_v4(),
				timestamp: chrono::Utc::now(),
				source: RequestSource::Internal,
				client_ip: None,
				user_agent: None,
			},
			device_context: DeviceContext {
				device_id,
				device_name,
				os: std::env::consts::OS.to_string(),
				hardware_model: "Unknown".to_string(),
				capabilities: DeviceCapabilities {
					supports_background_jobs: true,
					supports_networking: true,
					supports_file_operations: true,
					available_storage: None,
				},
			},
		}
	}

	/// Set the current library for this session
	pub fn with_library(mut self, library_id: Uuid) -> Self {
		self.current_library_id = Some(library_id);
		self
	}

	/// Check if session has the required authentication level
	pub fn has_auth_level(&self, required: AuthLevel) -> bool {
		self.auth.authentication_level >= required
	}
}

// PermissionSet impl methods are in permissions.rs
