//! Library rename action handler

use super::output::LibraryRenameOutput;
use crate::{
	context::CoreContext,
	infra::action::{error::ActionError, LibraryAction},
	library::LibraryConfig,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryRenameAction {
	pub library_id: Uuid,
	pub new_name: String,
}

impl LibraryRenameAction {
	/// Create a new library rename action
	pub fn new(library_id: Uuid, new_name: String) -> Self {
		Self {
			library_id,
			new_name,
		}
	}
}

// Old ActionHandler implementation removed - using unified ActionTrait

// Implement the new modular ActionType trait
impl LibraryAction for LibraryRenameAction {
	type Output = LibraryRenameOutput;

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
				config.name = self.new_name.clone();
			})
			.await
			.map_err(|e| ActionError::Internal(format!("Failed to save config: {}", e)))?;

		// Return native output directly
		Ok(LibraryRenameOutput {
			library_id: self.library_id,
			old_name,
			new_name: self.new_name,
		})
	}

	fn action_kind(&self) -> &'static str {
		"library.rename"
	}

	async fn validate(
		&self,
		library: &std::sync::Arc<crate::library::Library>,
		context: std::sync::Arc<CoreContext>,
	) -> Result<(), ActionError> {
		// Library existence already validated by ActionManager - no boilerplate!

		// Validate new name
		if self.new_name.trim().is_empty() {
			return Err(ActionError::Validation {
				field: "new_name".to_string(),
				message: "Library name cannot be empty".to_string(),
			});
		}

		Ok(())
	}
}
