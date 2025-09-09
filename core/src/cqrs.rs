//! CQRS (Command Query Responsibility Segregation) traits for Spacedrive Core
//!
//! This module provides the foundational traits for the enhanced Core API:
//! - Command trait for write/mutation operations with modular outputs
//! - Query trait for read-only operations
//!
//! This modular approach eliminates the centralized ActionOutput enum,
//! allowing each operation to own its output type completely.

use crate::context::CoreContext;
use anyhow::Result;
use std::sync::Arc;

/// A command that mutates system state with modular output types.
///
/// Each command defines its own native output type, eliminating the need
/// for centralized enums and JSON serialization round-trips.
pub trait Command {
	/// The output after the command succeeds (owned by the operation module).
	type Output: Send + Sync + 'static;

	/// Execute this command directly, returning its native output type.
	///
	/// Implementations should preserve existing business logic while
	/// returning native output types instead of going through ActionOutput.
	async fn execute(self, context: Arc<CoreContext>) -> Result<Self::Output>;
}

/// Execute any command directly with full type safety.
///
/// This function provides a unified entry point while preserving
/// native output types throughout the execution chain.
pub async fn execute_command<C: Command>(
	command: C,
	context: Arc<CoreContext>,
) -> Result<C::Output> {
	// Direct execution - no ActionOutput enum conversion!
	command.execute(context).await
}

/// A request that retrieves data without mutating state.
///
/// This trait provides the foundation for a formal query system
/// that mirrors the robustness of the existing action system.
pub trait Query {
	/// The data structure returned by the query (owned by the operation module).
	type Output: Send + Sync + 'static;

	/// Execute this query with the given context.
	///
	/// Query implementations can access any services through the context
	/// and should focus on read-only operations.
	async fn execute(self, context: Arc<CoreContext>) -> Result<Self::Output>;
}