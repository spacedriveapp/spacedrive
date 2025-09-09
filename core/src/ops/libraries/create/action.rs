//! Library creation action handler

use super::output::LibraryCreateOutput;
use crate::{
	context::CoreContext,
	infra::action::{
		error::ActionError,
		CoreAction,
	},
};
use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LibraryCreateAction {
	pub name: String,
	pub path: Option<PathBuf>,
}

// LibraryCreateHandler removed - using unified ActionTrait instead

// Implement the new modular ActionType trait
impl CoreAction for LibraryCreateAction {
	type Output = LibraryCreateOutput;

	async fn execute(self, context: Arc<CoreContext>) -> Result<Self::Output, ActionError> {
		// Delegate to existing business logic
		let library_manager = &context.library_manager;
		let library = library_manager
			.create_library(self.name.clone(), self.path.clone(), context.clone())
			.await?;

		// Return native output directly - no ActionOutput conversion!
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
		if self.name.trim().is_empty() {
			return Err(ActionError::Validation {
				field: "name".to_string(),
				message: "Library name cannot be empty".to_string(),
			});
		}
		Ok(())
	}
}

