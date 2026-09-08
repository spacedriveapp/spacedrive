//! Source sync action — dispatches a SourceSyncJob

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
		_context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		// The name is for the job's own display; the job re-reads the row it
		// needs when it runs.
		let source_name = match uuid::Uuid::parse_str(&self.input.source_id) {
			Ok(id) => registry::get(library.db().conn(), id)
				.await
				.map(|row| row.name)
				.unwrap_or_else(|_| self.input.source_id.clone()),
			Err(_) => self.input.source_id.clone(),
		};

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
