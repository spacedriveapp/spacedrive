//! Library rename action handler

use super::output::LibraryRenameOutput;
use crate::{
	context::CoreContext,
	infra::action::{error::ActionError, LibraryAction},
	library::LibraryConfig,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LibraryRenameInput {
	pub library_id: Uuid,
	pub new_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryRenameAction {
	input: LibraryRenameInput,
}

impl LibraryRenameAction {
	/// Create a new library rename action
	pub fn new(input: LibraryRenameInput) -> Self {
		Self { input }
	}
}

// Old ActionHandler implementation removed - using unified ActionTrait

// Implement the new modular ActionType trait
impl LibraryAction for LibraryRenameAction {
	type Input = LibraryRenameInput;
	type Output = LibraryRenameOutput;

	fn from_input(input: LibraryRenameInput) -> Result<Self, String> {
		Ok(LibraryRenameAction::new(input))
	}

	async fn execute(
		self,
		library: std::sync::Arc<crate::library::Library>,
		context: std::sync::Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		// Library is pre-validated by ActionManager - no boilerplate!

		// Get current config
		let old_config = library.config().await;
		let old_name = old_config.name.clone();

		// Update the library name using update_config
		library
			.update_config(|config| {
				config.name = self.input.new_name.clone();
			})
			.await
			.map_err(|e| ActionError::Internal(format!("Failed to save config: {}", e)))?;

		// Return native output directly
		Ok(LibraryRenameOutput {
			library_id: self.input.library_id,
			old_name,
			new_name: self.input.new_name,
		})
	}

	fn action_kind(&self) -> &'static str {
		"library.rename"
	}

	async fn validate(
		&self,
		_library: std::sync::Arc<crate::library::Library>,
		_context: std::sync::Arc<crate::context::CoreContext>,
	) -> Result<(), ActionError> {
		// Library existence already validated by ActionManager - no boilerplate!

		// Validate new name
		if self.input.new_name.trim().is_empty() {
			return Err(ActionError::Validation {
				field: "new_name".to_string(),
				message: "Library name cannot be empty".to_string(),
			});
		}

		Ok(())
	}
}

// Register action
crate::register_library_action!(LibraryRenameAction, "libraries.rename");
