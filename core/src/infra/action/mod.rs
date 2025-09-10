//! Action System - User-initiated operations with audit logging
//!
//! This module provides a centralized, robust, and extensible layer for handling
//! all user-initiated operations. It serves as the primary integration point
//! for the CLI and future APIs.

use crate::domain::addressing::SdPath;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

pub mod builder;
pub mod error;
pub mod manager;
pub mod output;
pub mod receipt;

// handler and registry modules removed - using unified ActionTrait instead

/// Core-level action that operates without library context.
///
/// These actions work at the global level - managing libraries themselves,
/// volumes, devices, etc. They don't require a specific library context.
pub trait CoreAction: Send + Sync + 'static {
	/// The output type for this action - can be domain objects, job handles, etc.
	type Output: Send + Sync + 'static;

	/// Execute this action with core context only
	async fn execute(
		self,
		context: std::sync::Arc<crate::context::CoreContext>,
	) -> Result<Self::Output, crate::infra::action::error::ActionError>;

	/// Get the action kind for logging/identification
	fn action_kind(&self) -> &'static str;

	/// Validate this action (optional)
	async fn validate(
		&self,
		_context: std::sync::Arc<crate::context::CoreContext>,
	) -> Result<(), crate::infra::action::error::ActionError> {
		Ok(())
	}
}

/// Library-scoped action that operates within a specific library context.
///
/// These actions work on files, locations, indexing, etc. within a library.
/// The ActionManager validates library existence and provides the Library object directly.
pub trait LibraryAction: Send + Sync + 'static {
	/// The output type for this action - can be domain objects, job handles, etc.
	type Output: Send + Sync + 'static;

	/// Execute this action with validated library and core context
	async fn execute(
		self,
		library: std::sync::Arc<crate::library::Library>,
		context: std::sync::Arc<crate::context::CoreContext>,
	) -> Result<Self::Output, crate::infra::action::error::ActionError>;

	/// Get the action kind for logging/identification
	fn action_kind(&self) -> &'static str;

	/// Get the library ID for this action (required)
	fn library_id(&self) -> Uuid;

	/// Validate this action with library context (optional)
	/// Note: Library existence is already validated by ActionManager
	async fn validate(
		&self,
		_library: &std::sync::Arc<crate::library::Library>,
		_context: std::sync::Arc<crate::context::CoreContext>,
	) -> Result<(), crate::infra::action::error::ActionError> {
		Ok(())
	}
}
