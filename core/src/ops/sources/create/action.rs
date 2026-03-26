//! Source creation action handler

use super::{input::CreateSourceInput, output::CreateSourceOutput};
use crate::{
	context::CoreContext,
	infra::action::{error::ActionError, LibraryAction},
	library::Library,
};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CreateSourceAction {
	input: CreateSourceInput,
}

impl CreateSourceAction {
	pub fn new(input: CreateSourceInput) -> Self {
		Self { input }
	}
}

impl LibraryAction for CreateSourceAction {
	type Input = CreateSourceInput;
	type Output = CreateSourceOutput;

	fn from_input(input: CreateSourceInput) -> Result<Self, String> {
		if input.name.trim().is_empty() {
			return Err("Source name cannot be empty".to_string());
		}
		if input.adapter_id.trim().is_empty() {
			return Err("Adapter ID cannot be empty".to_string());
		}
		Ok(CreateSourceAction::new(input))
	}

	async fn execute(
		self,
		library: Arc<Library>,
		_context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		// Get or initialize the source manager
		if library.source_manager().is_none() {
			library.init_source_manager().await.map_err(|e| {
				ActionError::Internal(format!("Failed to init source manager: {e}"))
			})?;
		}

		let source_manager = library
			.source_manager()
			.ok_or_else(|| ActionError::Internal("Source manager not available".to_string()))?;

		// Create the source via sd-archive
		let source_info = source_manager
			.create_source(&self.input.name, &self.input.adapter_id, self.input.config)
			.await
			.map_err(|e| ActionError::Internal(format!("Failed to create source: {e}")))?;

		let source_id = Uuid::parse_str(&source_info.id)
			.map_err(|e| ActionError::Internal(format!("Invalid source ID: {e}")))?;

		Ok(CreateSourceOutput::new(
			source_id,
			source_info.name,
			source_info.adapter_id,
			source_info.status,
		))
	}

	fn action_kind(&self) -> &'static str {
		"sources.create"
	}
}

// Register library-scoped action
crate::register_library_action!(CreateSourceAction, "sources.create");
