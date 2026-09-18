//! Merge assertions delivered by another device into a source this device
//! owns.
//!
//! Invoked over the paired-device remote operation path by the author's
//! outbox. Commit happens before the reply, so the reply is the ack; the
//! assertion primary key absorbs redelivery after a lost one. Received
//! stamps fold into the local clock, so a removal authored here afterwards
//! sorts after the applies that just arrived.

use super::{input::MergeAssertionsInput, output::MergeAssertionsOutput};
use crate::{
	context::CoreContext,
	infra::action::{error::ActionError, LibraryAction},
	infra::sync::HLC,
	library::Library,
	ops::tags::stamp,
};
use sd_store::{TagAssertion, TagDefinition};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeAssertionsAction {
	input: MergeAssertionsInput,
}

impl LibraryAction for MergeAssertionsAction {
	type Input = MergeAssertionsInput;
	type Output = MergeAssertionsOutput;

	fn from_input(input: MergeAssertionsInput) -> Result<Self, String> {
		input.validate()?;
		Ok(Self { input })
	}

	async fn execute(
		self,
		_library: Arc<Library>,
		context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let cache = context.volume_index();

		// Ownership is the trust decision: only a source registered on this
		// machine takes writes here, so a replica can never be altered by a
		// peer's delivery.
		let Some(root) = cache.source_root(self.input.source_uuid) else {
			return Err(ActionError::InvalidInput(format!(
				"this device does not own source {}",
				self.input.source_uuid
			)));
		};
		let Some(store) = cache.store_for(&root).await else {
			return Err(ActionError::Internal(format!(
				"store for source {} is unavailable",
				self.input.source_uuid
			)));
		};

		let definitions: Vec<TagDefinition> =
			self.input.definitions.iter().map(Into::into).collect();
		let assertions: Vec<TagAssertion> = self.input.assertions.iter().map(Into::into).collect();

		let device = context
			.device_manager
			.device_id()
			.map_err(|e| ActionError::Internal(format!("no device identity: {e}")))?;
		for assertion in &assertions {
			if let Ok(received) = HLC::from_string(&assertion.stamp.hlc) {
				stamp::observe_remote(device, received);
			}
		}

		store
			.db()
			.upsert_tag_definitions(&definitions)
			.await
			.map_err(|e| ActionError::Internal(format!("definition write failed: {e}")))?;
		let appended = store
			.db()
			.append_tag_assertions(&assertions)
			.await
			.map_err(|e| ActionError::Internal(format!("assertion write failed: {e}")))?;

		// A row authored against a replica may not know the content key; this
		// store does, so bind it now rather than waiting for the next hash.
		if let Err(error) = store.db().bind_assertion_content().await {
			tracing::warn!(%error, "assertion content binding failed");
		}

		let affected: Vec<uuid::Uuid> = assertions.iter().map(|a| a.record_uuid).collect();
		crate::domain::File::announce(&context, affected).await;

		Ok(MergeAssertionsOutput {
			definitions_received: definitions.len() as u32,
			assertions_appended: appended,
		})
	}

	fn action_kind(&self) -> &'static str {
		"sources.assertions.merge"
	}
}

crate::register_library_action!(MergeAssertionsAction, "sources.assertions.merge");
