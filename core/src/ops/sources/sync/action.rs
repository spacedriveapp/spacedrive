//! Source sync action handler

use crate::{
	context::CoreContext,
	infra::action::{error::ActionError, LibraryAction},
	library::Library,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SyncSourceInput {
	pub source_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SyncSourceOutput {
	pub records_upserted: u64,
	pub records_deleted: u64,
	pub duration_ms: u64,
	pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncSourceAction {
	input: SyncSourceInput,
}

impl LibraryAction for SyncSourceAction {
	type Input = SyncSourceInput;
	type Output = SyncSourceOutput;

	fn from_input(input: SyncSourceInput) -> Result<Self, String> {
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

		let report = source_manager
			.sync_source(&self.input.source_id)
			.await
			.map_err(|e| ActionError::Internal(e))?;

		Ok(SyncSourceOutput {
			records_upserted: report.records_upserted,
			records_deleted: report.records_deleted,
			duration_ms: report.duration_ms,
			error: report.error,
		})
	}

	fn action_kind(&self) -> &'static str {
		"sources.sync"
	}
}

crate::register_library_action!(SyncSourceAction, "sources.sync");
