//! Library creation action handler

use super::output::LibraryCreateOutput;
use crate::{
	context::CoreContext,
	cqrs::Command,
	infra::action::{
		error::{ActionError, ActionResult},
		handler::ActionHandler,
		output::ActionOutput,
	},
	register_action_handler,
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

pub struct LibraryCreateHandler;

impl LibraryCreateHandler {
	pub fn new() -> Self {
		Self
	}
}

#[async_trait]
impl ActionHandler for LibraryCreateHandler {
	async fn validate(
		&self,
		_context: Arc<CoreContext>,
		action: &crate::infra::action::Action,
	) -> ActionResult<()> {
		if let crate::infra::action::Action::LibraryCreate(action) = action {
			if action.name.trim().is_empty() {
				return Err(ActionError::Validation {
					field: "name".to_string(),
					message: "Library name cannot be empty".to_string(),
				});
			}
			Ok(())
		} else {
			Err(ActionError::InvalidActionType)
		}
	}

	async fn execute(
		&self,
		context: Arc<CoreContext>,
		action: crate::infra::action::Action,
	) -> ActionResult<ActionOutput> {
		if let crate::infra::action::Action::LibraryCreate(action) = action {
			let library_manager = &context.library_manager;
			let new_library = library_manager
				.create_library(action.name.clone(), action.path.clone(), context.clone())
				.await?;

			let library_name = new_library.name().await;
			let output = LibraryCreateOutput::new(
				new_library.id(),
				library_name,
				new_library.path().to_path_buf(),
			);
			Ok(ActionOutput::from_trait(output))
		} else {
			Err(ActionError::InvalidActionType)
		}
	}

	fn can_handle(&self, action: &crate::infra::action::Action) -> bool {
		matches!(action, crate::infra::action::Action::LibraryCreate(_))
	}

	fn supported_actions() -> &'static [&'static str] {
		&["library.create"]
	}
}

// Register this handler
register_action_handler!(LibraryCreateHandler, "library.create");

// Implement the modular Command trait for LibraryCreateAction
impl Command for LibraryCreateAction {
	type Output = LibraryCreateOutput;

	async fn execute(self, context: Arc<CoreContext>) -> anyhow::Result<Self::Output> {
		// Delegate to existing business logic while preserving audit logging
		let library_manager = &context.library_manager;
		let library = library_manager
			.create_library(self.name.clone(), self.path.clone(), context.clone())
			.await
			.map_err(|e| anyhow::anyhow!("Failed to create library: {}", e))?;

		// Return native output directly - no ActionOutput conversion!
		Ok(LibraryCreateOutput::new(
			library.id(),
			library.name().await,
			library.path().to_path_buf(),
		))
	}
}
