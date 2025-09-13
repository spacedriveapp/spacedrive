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
	/// The associated input type (wire contract) for this action
	type Input: Send + Sync + 'static;

	/// Build this action from its associated input
	fn from_input(input: Self::Input) -> Result<Self, String>
	where
		Self: Sized;

	/// Execute this action with core context only
	fn execute(
		self,
		context: std::sync::Arc<crate::context::CoreContext>,
	) -> impl std::future::Future<
		Output = Result<Self::Output, crate::infra::action::error::ActionError>,
	> + Send;

	/// Get the action kind for logging/identification
	fn action_kind(&self) -> &'static str;

	/// Validate this action (optional)
	fn validate(
		&self,
		_context: std::sync::Arc<crate::context::CoreContext>,
	) -> impl std::future::Future<Output = Result<(), crate::infra::action::error::ActionError>> + Send
	{
		async { Ok(()) }
	}
}

/// Library-scoped action that operates within a specific library context.
///
/// These actions work on files, locations, indexing, etc. within a library.
/// The ActionManager validates library existence and provides the Library object directly.
pub trait LibraryAction: Send + Sync + 'static {
	/// The output type for this action - can be domain objects, job handles, etc.
	type Output: Send + Sync + 'static;
	/// The associated input type (wire contract) for this action
	type Input: Send + Sync + 'static;

	/// Build this action from its associated input
	fn from_input(input: Self::Input) -> Result<Self, String>
	where
		Self: Sized;

	/// Execute this action with validated library and core context
	fn execute(
		self,
		library: std::sync::Arc<crate::library::Library>,
		context: std::sync::Arc<crate::context::CoreContext>,
	) -> impl std::future::Future<
		Output = Result<Self::Output, crate::infra::action::error::ActionError>,
	> + Send;

	/// Get the action kind for logging/identification
	fn action_kind(&self) -> &'static str;

	/// Validate this action with library context (optional)
	/// Note: Library existence is already validated by ActionManager
	fn validate(
		&self,
		_library: &std::sync::Arc<crate::library::Library>,
		_context: std::sync::Arc<crate::context::CoreContext>,
	) -> impl std::future::Future<Output = Result<(), crate::infra::action::error::ActionError>> + Send
	{
		async { Ok(()) }
	}
}
