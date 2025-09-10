//! Library deletion action handler

use super::output::LibraryDeleteOutput;
use crate::{
	context::CoreContext,
	infra::action::{error::ActionError, CoreAction},
	ops::libraries::LibraryDeleteInput,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryDeleteAction {
	input: LibraryDeleteInput,
}

impl LibraryDeleteAction {
	pub fn new(input: LibraryDeleteInput) -> Self {
		Self { input }
	}
}

pub struct LibraryDeleteHandler {
	input: LibraryDeleteInput,
}

impl LibraryDeleteHandler {
	pub fn new(input: LibraryDeleteInput) -> Self {
		Self { input }
	}
}

impl CoreAction for LibraryDeleteAction {
	type Input = LibraryDeleteInput;
	type Output = LibraryDeleteOutput;

	fn from_input(input: LibraryDeleteInput) -> Result<Self, String> {
		Ok(LibraryDeleteAction::new(input))
	}

	async fn execute(
		self,
		context: std::sync::Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		// Get the library to get its name before deletion
		let library = context
			.library_manager
			.get_library(self.input.library_id)
			.await
			.ok_or_else(|| ActionError::LibraryNotFound(self.input.library_id))?;

		let library_name = library.name().await;

		// Delete the library through the library manager
		context
			.library_manager
			.delete_library(self.input.library_id, self.input.delete_data)
			.await?;

		// Return native output directly
		Ok(LibraryDeleteOutput::new(
			self.input.library_id,
			library_name,
		))
	}

	fn action_kind(&self) -> &'static str {
		"library.delete"
	}

	// No library_id method - this is a CoreAction that operates on libraries themselves

	async fn validate(&self, _context: std::sync::Arc<CoreContext>) -> Result<(), ActionError> {
		// Basic validation
		Ok(())
	}
}

// Register core action
crate::register_core_action!(LibraryDeleteAction, "libraries.delete");
