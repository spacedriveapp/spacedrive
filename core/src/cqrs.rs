//! CQRS (Command Query Responsibility Segregation) traits for Spacedrive Core
//!
//! This module provides the foundational traits for the enhanced Core API:
//! - Command trait for write/mutation operations
//! - Query trait for read-only operations
//!
//! These traits work alongside the existing ActionManager infrastructure,
//! providing a unified, type-safe entry point for all Core operations.

use crate::{
	context::CoreContext,
	infra::action::{output::ActionOutput, Action},
};
use anyhow::Result;
use std::sync::Arc;

/// A command that mutates system state.
///
/// This trait provides a unified API for existing action structs while
/// preserving all existing ActionManager functionality including audit
/// logging, validation, and error handling.
pub trait Command {
	/// The output after the command succeeds.
	type Output;

	/// Convert this command into the corresponding Action enum variant.
	/// This allows the generic implementation to work with any action.
	fn into_action(self) -> Action;

	/// Extract the typed output from the generic ActionOutput.
	/// This handles the conversion from the action system's output format.
	fn extract_output(output: ActionOutput) -> Result<Self::Output>;
}

/// Execute any command through the existing ActionManager infrastructure.
pub async fn execute_command<C: Command>(
	command: C,
	context: Arc<CoreContext>,
) -> Result<C::Output> {
	// Convert to Action enum
	let action = command.into_action();

	// Get the action manager from context
	let action_manager = context
		.get_action_manager()
		.await
		.ok_or_else(|| anyhow::anyhow!("ActionManager not initialized"))?;

	// Dispatch through existing infrastructure
	let result = action_manager.dispatch(action).await?;

	// Convert output back to typed result
	C::extract_output(result)
}

/// A request that retrieves data without mutating state.
///
/// This trait provides the foundation for a formal query system
/// that mirrors the robustness of the existing action system.
pub trait Query {
	/// The data structure returned by the query.
	type Output;

	/// Execute this query with the given context.
	///
	/// Query implementations can access any services through the context
	/// and should focus on read-only operations.
	async fn execute(self, context: Arc<CoreContext>) -> Result<Self::Output>;
}
