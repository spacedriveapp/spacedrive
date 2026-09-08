//! Source creation action handler

use super::{input::CreateSourceInput, output::CreateSourceOutput};
use crate::{
	context::CoreContext,
	infra::action::{error::ActionError, LibraryAction},
	library::Library,
	ops::sources::registry,
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

		// The identity is minted here, because the registration is written
		// here. The engine is told what to call the store.
		let source_id = Uuid::now_v7();
		let facts = source_manager
			.create_source(&registry::store_id(source_id), &self.input.adapter_id)
			.await
			.map_err(|e| ActionError::Internal(format!("Failed to create source: {e}")))?;

		let row = registry::register(
			library.db().conn(),
			source_id,
			&self.input.name,
			&self.input.adapter_id,
			&self.input.config,
			&facts,
		)
		.await
		.map_err(|e| ActionError::Internal(format!("Failed to register source: {e}")))?;

		Ok(CreateSourceOutput::new(
			source_id,
			row.name,
			self.input.adapter_id,
			row.status,
		))
	}

	fn action_kind(&self) -> &'static str {
		"sources.create"
	}
}

// Register library-scoped action
crate::register_library_action!(CreateSourceAction, "sources.create");
