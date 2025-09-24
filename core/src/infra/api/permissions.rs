//! Permission and authorization layer
//!
//! This module handles authentication and authorization for all API operations.
//! It provides fine-grained control over what operations each session can execute.

use super::{error::ApiError, session::SessionContext};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;
use thiserror::Error;
use uuid::Uuid;

pub use super::session::AuthLevel;

/// Permission checking and enforcement
#[derive(Clone)]
pub struct PermissionLayer {
	/// Permission policies and rules
	policies: PermissionPolicies,
}

/// Permission policies configuration
#[derive(Debug, Clone)]
pub struct PermissionPolicies {
	/// Whether to enforce permissions (can disable for development)
	pub enforce: bool,

	/// Default permissions for device-level access
	pub device_defaults: PermissionSet,

	/// Library-specific permission overrides
	pub library_overrides: HashMap<Uuid, LibraryPermissions>,
}

/// Complete permission set for a session
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct PermissionSet {
	/// Core system permissions
	pub core: CorePermissions,

	/// Library operation permissions
	pub library: LibraryPermissions,

	/// Network operation permissions
	pub network: NetworkPermissions,

	/// Job management permissions
	pub jobs: JobPermissions,
}

/// Core system permissions
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CorePermissions {
	pub can_read_status: bool,
	pub can_manage_libraries: bool,
	pub can_modify_settings: bool,
	pub can_manage_devices: bool,
}

/// Library operation permissions
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LibraryPermissions {
	pub can_read: bool,
	pub can_write: bool,
	pub can_delete: bool,
	pub can_manage_locations: bool,
	pub can_manage_tags: bool,
	pub can_search: bool,
	pub can_index: bool,
}

/// Network operation permissions
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct NetworkPermissions {
	pub can_start_stop: bool,
	pub can_pair_devices: bool,
	pub can_send_spacedrop: bool,
	pub can_manage_devices: bool,
}

/// Job management permissions
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct JobPermissions {
	pub can_list: bool,
	pub can_pause_resume: bool,
	pub can_cancel: bool,
	pub can_view_details: bool,
}

/// Permission-related errors
#[derive(Debug, Error, Serialize, Deserialize, Type)]
pub enum PermissionError {
	#[error("Authentication required")]
	Unauthenticated,

	#[error("Insufficient privileges for this operation")]
	InsufficientPrivileges,

	#[error("Library access denied: {library_id}")]
	LibraryAccessDenied { library_id: Uuid },

	#[error("Operation not allowed: {operation}")]
	OperationNotAllowed { operation: String },

	#[error("Rate limit exceeded")]
	RateLimitExceeded,
}

impl PermissionLayer {
	/// Create a new permission layer with default policies
	pub fn new() -> Self {
		Self {
			policies: PermissionPolicies {
				enforce: true,
				device_defaults: PermissionSet::device_default(),
				library_overrides: HashMap::new(),
			},
		}
	}

	/// Create a permissive layer for development
	pub fn permissive() -> Self {
		Self {
			policies: PermissionPolicies {
				enforce: false,
				device_defaults: PermissionSet::admin_all(),
				library_overrides: HashMap::new(),
			},
		}
	}

	/// Check if session can execute a library action
	pub async fn check_library_action<A>(
		&self,
		session: &SessionContext,
		_action_type: std::marker::PhantomData<A>,
	) -> Result<(), PermissionError>
	where
		A: crate::infra::action::LibraryAction,
	{
		if !self.policies.enforce {
			return Ok(());
		}

		// Check basic authentication
		if session.auth.authentication_level == AuthLevel::None {
			return Err(PermissionError::Unauthenticated);
		}

		// Check library access
		if session.current_library_id.is_none() {
			return Err(PermissionError::OperationNotAllowed {
				operation: "Library action requires library context".to_string(),
			});
		}

		// Check library permissions
		if !session.permissions.library.can_write {
			return Err(PermissionError::InsufficientPrivileges);
		}

		// Future: More fine-grained checks based on action type
		// match std::any::type_name::<A>() {
		//     "FileCopyAction" => check_file_permissions(),
		//     "IndexingAction" => check_indexing_permissions(),
		//     _ => Ok(()),
		// }

		Ok(())
	}

	/// Check if session can execute a core action
	pub async fn check_core_action<A>(
		&self,
		session: &SessionContext,
		_action_type: std::marker::PhantomData<A>,
	) -> Result<(), PermissionError>
	where
		A: crate::infra::action::CoreAction,
	{
		if !self.policies.enforce {
			return Ok(());
		}

		// Core actions typically need higher privileges
		match session.auth.authentication_level {
			AuthLevel::Device | AuthLevel::User(_) | AuthLevel::Admin(_) => {
				if session.permissions.core.can_manage_libraries {
					Ok(())
				} else {
					Err(PermissionError::InsufficientPrivileges)
				}
			}
			AuthLevel::None => Err(PermissionError::Unauthenticated),
		}
	}

	/// Check if session can execute a library query
	pub async fn check_library_query<Q>(
		&self,
		session: &SessionContext,
		_query_type: std::marker::PhantomData<Q>,
	) -> Result<(), PermissionError>
	where
		Q: crate::cqrs::LibraryQuery,
	{
		if !self.policies.enforce {
			return Ok(());
		}

		// Queries typically need read permissions
		if session.auth.authentication_level == AuthLevel::None {
			return Err(PermissionError::Unauthenticated);
		}

		if !session.permissions.library.can_read {
			return Err(PermissionError::InsufficientPrivileges);
		}

		Ok(())
	}

	/// Check if session can execute a core query
	pub async fn check_core_query<Q>(
		&self,
		session: &SessionContext,
		_query_type: std::marker::PhantomData<Q>,
	) -> Result<(), PermissionError>
	where
		Q: crate::cqrs::CoreQuery,
	{
		if !self.policies.enforce {
			return Ok(());
		}

		// Core queries need basic authentication
		if session.auth.authentication_level == AuthLevel::None {
			return Err(PermissionError::Unauthenticated);
		}

		if !session.permissions.core.can_read_status {
			return Err(PermissionError::InsufficientPrivileges);
		}

		Ok(())
	}
}

impl PermissionSet {
	/// Full admin permissions
	pub fn admin_all() -> Self {
		Self {
			core: CorePermissions {
				can_read_status: true,
				can_manage_libraries: true,
				can_modify_settings: true,
				can_manage_devices: true,
			},
			library: LibraryPermissions {
				can_read: true,
				can_write: true,
				can_delete: true,
				can_manage_locations: true,
				can_manage_tags: true,
				can_search: true,
				can_index: true,
			},
			network: NetworkPermissions {
				can_start_stop: true,
				can_pair_devices: true,
				can_send_spacedrop: true,
				can_manage_devices: true,
			},
			jobs: JobPermissions {
				can_list: true,
				can_pause_resume: true,
				can_cancel: true,
				can_view_details: true,
			},
		}
	}

	/// Default device permissions (current behavior)
	pub fn device_default() -> Self {
		Self::admin_all() // For now, maintain current permissive behavior
	}

	/// Read-only permissions
	pub fn read_only() -> Self {
		Self {
			core: CorePermissions {
				can_read_status: true,
				can_manage_libraries: false,
				can_modify_settings: false,
				can_manage_devices: false,
			},
			library: LibraryPermissions {
				can_read: true,
				can_write: false,
				can_delete: false,
				can_manage_locations: false,
				can_manage_tags: false,
				can_search: true,
				can_index: false,
			},
			network: NetworkPermissions {
				can_start_stop: false,
				can_pair_devices: false,
				can_send_spacedrop: false,
				can_manage_devices: false,
			},
			jobs: JobPermissions {
				can_list: true,
				can_pause_resume: false,
				can_cancel: false,
				can_view_details: true,
			},
		}
	}
}

/// Convert permission errors to API errors
impl From<PermissionError> for ApiError {
	fn from(err: PermissionError) -> Self {
		match err {
			PermissionError::Unauthenticated => Self::Unauthenticated,
			PermissionError::InsufficientPrivileges => Self::InsufficientPermissions {
				reason: "Operation not allowed".to_string(),
			},
			PermissionError::LibraryAccessDenied { library_id } => Self::LibraryNotFound {
				library_id: library_id.to_string(),
			},
			PermissionError::OperationNotAllowed { operation } => {
				Self::InsufficientPermissions { reason: operation }
			}
			PermissionError::RateLimitExceeded => Self::RateLimitExceeded {
				retry_after_seconds: 60,
			},
		}
	}
}
