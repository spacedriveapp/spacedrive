//! Library creation action handler

use super::{input::LibraryCreateInput, output::LibraryCreateOutput};
use crate::{
	context::CoreContext,
	infra::action::{error::ActionError, CoreAction},
};
use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LibraryCreateAction {
	input: LibraryCreateInput,
}

impl LibraryCreateAction {
	pub fn new(input: LibraryCreateInput) -> Self {
		Self { input }
	}
}

// Implement the new modular ActionType trait
impl CoreAction for LibraryCreateAction {
	type Input = LibraryCreateInput;
	type Output = LibraryCreateOutput;

	fn from_input(input: LibraryCreateInput) -> Result<Self, String> {
		Ok(LibraryCreateAction::new(input))
	}

	async fn execute(self, context: Arc<CoreContext>) -> Result<Self::Output, ActionError> {
		let library_manager = &context.library_manager;
		let library = library_manager
			.create_library(
				self.input.name.clone(),
				self.input.path.clone(),
				context.clone(),
			)
			.await?;

		Ok(LibraryCreateOutput::new(
			library.id(),
			library.name().await,
			library.path().to_path_buf(),
		))
	}

	fn action_kind(&self) -> &'static str {
		"library.create"
	}

	async fn validate(&self, _context: Arc<CoreContext>) -> Result<(), ActionError> {
		if self.input.name.trim().is_empty() {
			return Err(ActionError::Validation {
				field: "name".to_string(),
				message: "Library name cannot be empty".to_string(),
			});
		}
		Ok(())
	}
}
