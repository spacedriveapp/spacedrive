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
#[cfg(test)]
mod tests;

// handler and registry modules removed - using unified ActionTrait instead


/// Unified action trait for all operations in Spacedrive.
///
/// This trait represents any user-initiated operation, whether it completes
/// immediately (returning domain objects) or dispatches background jobs
/// (returning job handles). The output type naturally reflects what the action produces.
pub trait ActionTrait: Send + Sync + 'static {
	/// The output type for this action - can be domain objects, job handles, etc.
	type Output: Send + Sync + 'static;

	/// Execute this action and return its natural output
	async fn execute(self, context: std::sync::Arc<crate::context::CoreContext>) -> Result<Self::Output, crate::infra::action::error::ActionError>;

	/// Get the action kind for logging/identification
	fn action_kind(&self) -> &'static str;

	/// Get the library ID if this action is library-scoped (optional)
	fn library_id(&self) -> Option<Uuid> {
		None
	}

	/// Validate this action (optional)
	async fn validate(&self, _context: std::sync::Arc<crate::context::CoreContext>) -> Result<(), crate::infra::action::error::ActionError> {
		Ok(())
	}
}

// Action enum removed! 🎉
// Actions are now dispatched directly through ActionTrait without central enumeration.
// This eliminates the centralization problem while preserving central dispatch infrastructure.

// All Action enum methods removed with the enum itself!
