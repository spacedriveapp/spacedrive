//! Dispatch full-read verification of a source's shared content.
//!
//! The sampled tier groups candidates cheaply during indexing; this is the
//! deliberate, expensive step behind any decision that deletes bytes. It runs
//! per source, and the job claims its work from the store, so what gets read
//! is exactly the files whose content is shared and unverified at the moment
//! each batch is claimed.

use crate::{
	context::CoreContext,
	infra::action::{error::ActionError, LibraryAction},
	library::Library,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct VerifySourceInput {
	pub source_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct VerifySourceOutput {
	pub job_id: uuid::Uuid,
	/// Shared-content files awaiting verification when the job was queued.
	/// The job re-claims as it runs, so the final count can be higher.
	pub outstanding: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifySourceAction {
	input: VerifySourceInput,
}

impl LibraryAction for VerifySourceAction {
	type Input = VerifySourceInput;
	type Output = VerifySourceOutput;

	fn from_input(input: VerifySourceInput) -> Result<Self, String> {
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
		let source_id = uuid::Uuid::parse_str(&self.input.source_id)
			.map_err(|e| ActionError::Internal(format!("Invalid source ID: {e}")))?;

		// The registry holds the source's absolute root; the library row only
		// stores it relative to its volume.
		let root = context
			.volume_index()
			.source_root(source_id)
			.ok_or_else(|| {
				ActionError::Internal(format!(
					"source {source_id} is not registered on this machine"
				))
			})?;

		let outstanding = match context.volume_index().store_for(&root).await {
			Some(store) => store
				.files_needing_verification_count()
				.await
				.map_err(|e| ActionError::Internal(e.to_string()))?,
			None => 0,
		};

		let job = crate::ops::indexing::verify_content::VerifyContentJob::new(root);
		let handle = library
			.jobs()
			.dispatch(job)
			.await
			.map_err(|e| ActionError::Internal(format!("could not start verification: {e}")))?;

		Ok(VerifySourceOutput {
			job_id: handle.id().0,
			outstanding,
		})
	}

	fn action_kind(&self) -> &'static str {
		"sources.verify"
	}
}

crate::register_library_action!(VerifySourceAction, "sources.verify");
