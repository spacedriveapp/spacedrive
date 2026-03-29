//! Source deletion action handler

use crate::{
	context::CoreContext,
	infra::action::{error::ActionError, LibraryAction},
	library::Library,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DeleteSourceInput {
	pub source_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DeleteSourceOutput {
	pub deleted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteSourceAction {
	input: DeleteSourceInput,
}

impl LibraryAction for DeleteSourceAction {
	type Input = DeleteSourceInput;
	type Output = DeleteSourceOutput;

	fn from_input(input: DeleteSourceInput) -> Result<Self, String> {
		if input.source_id.trim().is_empty() {
			return Err("Source ID cannot be empty".to_string());
		}
		Ok(Self { input })
	}

	async fn execute(
		self,
		library: Arc<Library>,
		_context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		if library.source_manager().is_none() {
			library.init_source_manager().await.map_err(|e| {
				ActionError::Internal(format!("Failed to init source manager: {e}"))
			})?;
		}

		let source_manager = library
			.source_manager()
			.ok_or_else(|| ActionError::Internal("Source manager not available".to_string()))?;

		source_manager
			.delete_source(&self.input.source_id)
			.await
			.map_err(|e| ActionError::Internal(e))?;

		Ok(DeleteSourceOutput { deleted: true })
	}

	fn action_kind(&self) -> &'static str {
		"sources.delete"
	}
}

crate::register_library_action!(DeleteSourceAction, "sources.delete");
