//! Refresh a source through its filesystem walk or adapter.

use super::job::SourceSyncJob;
use crate::{
	context::CoreContext,
	infra::action::{error::ActionError, LibraryAction},
	library::Library,
	ops::sources::registry,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SyncSourceInput {
	pub source_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncSourceAction {
	input: SyncSourceInput,
}

impl LibraryAction for SyncSourceAction {
	type Input = SyncSourceInput;
	type Output = crate::infra::job::handle::JobReceipt;

	fn from_input(input: SyncSourceInput) -> Result<Self, String> {
		if input.source_id.trim().is_empty() {
			return Err("Source ID cannot be empty".to_string());
		}
		Ok(Self { input })
	}

	async fn execute(
		self,
		library: Arc<Library>,
		context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let id = uuid::Uuid::parse_str(&self.input.source_id)
			.map_err(|e| ActionError::InvalidInput(format!("Invalid source ID: {e}")))?;
		let source = registry::get(library.db().conn(), id)
			.await
			.map_err(|e| ActionError::InvalidInput(e.to_string()))?;

		if source.data_type == registry::FILESYSTEM {
			let root = context.volume_index().source_root(id).ok_or_else(|| {
				ActionError::InvalidInput("Source has no root on this machine".to_string())
			})?;
			let output = crate::ops::sources::track::action::track_and_index(
				&library,
				&context,
				root,
				None,
				&crate::library::config::AddOverrides::default(),
			)
			.await?;
			let job_id = output.job_id.ok_or_else(|| {
				ActionError::Internal("Could not start the source walk".to_string())
			})?;
			return Ok(crate::infra::job::handle::JobReceipt {
				id: crate::infra::job::types::JobId(job_id),
				job_name: "indexer".to_string(),
			});
		}
		let source_name = source.name;

		let job = SourceSyncJob::new(self.input.source_id, source_name);

		let job_handle = library
			.jobs()
			.dispatch(job)
			.await
			.map_err(ActionError::Job)?;

		Ok(job_handle.into())
	}

	fn action_kind(&self) -> &'static str {
		"sources.sync"
	}
}

crate::register_library_action!(SyncSourceAction, "sources.sync");
