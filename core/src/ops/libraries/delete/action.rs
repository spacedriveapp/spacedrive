//! Library deletion action handler

use super::output::LibraryDeleteOutput;
use crate::{
	context::CoreContext,
	infra::action::{
		error::ActionError,
		CoreAction,
	},
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryDeleteAction {
	pub library_id: Uuid,
}

pub struct LibraryDeleteHandler;

impl LibraryDeleteHandler {
	pub fn new() -> Self {
		Self
	}
}

// Note: ActionHandler implementation removed - using ActionType instead

// Implement the new modular ActionType trait
impl CoreAction for LibraryDeleteAction {
	type Output = LibraryDeleteOutput;

	async fn execute(self, context: std::sync::Arc<CoreContext>) -> Result<Self::Output, ActionError> {
		// Get the library to get its name before deletion
		let library = context
			.library_manager
			.get_library(self.library_id)
			.await
			.ok_or_else(|| ActionError::LibraryNotFound(self.library_id))?;

		let library_name = library.name().await;

		// Delete the library through the library manager
		// TODO: Implement actual deletion - for now just return success
		// context.library_manager.delete_library(self.library_id).await?;

		// Return native output directly
		Ok(LibraryDeleteOutput::new(self.library_id, library_name))
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
