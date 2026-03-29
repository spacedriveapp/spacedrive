//! Adapter update action handler

use crate::{
	context::CoreContext,
	infra::action::{error::ActionError, LibraryAction},
	library::Library,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct UpdateAdapterInput {
	pub adapter_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct UpdateAdapterOutput {
	pub adapter_id: String,
	pub old_version: String,
	pub new_version: String,
	pub schema_changed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateAdapterAction {
	input: UpdateAdapterInput,
}

impl LibraryAction for UpdateAdapterAction {
	type Input = UpdateAdapterInput;
	type Output = UpdateAdapterOutput;

	fn from_input(input: UpdateAdapterInput) -> Result<Self, String> {
		if input.adapter_id.trim().is_empty() {
			return Err("Adapter ID cannot be empty".to_string());
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

		let result = source_manager
			.update_adapter(&self.input.adapter_id)
			.map_err(|e| ActionError::Internal(e))?;

		Ok(UpdateAdapterOutput {
			adapter_id: result.adapter_id,
			old_version: result.old_version,
			new_version: result.new_version,
			schema_changed: result.schema_changed,
		})
	}

	fn action_kind(&self) -> &'static str {
		"adapters.update"
	}
}

crate::register_library_action!(UpdateAdapterAction, "adapters.update");
