//! Apply tags: append assertions into the stores that own the targets.
//!
//! A tag write is a row, stamped and device-attributed, so applying is
//! resolving targets to their stores, copying the definitions in, and
//! appending. Content-scoped changes reach every copy of the bytes, so every
//! copy is announced.

use super::{input::ApplyTagsInput, output::ApplyTagsOutput};
use crate::{
	context::CoreContext,
	infra::action::{error::ActionError, LibraryAction},
	library::Library,
	ops::tags::{definitions, merge, outbox, stamp, targets},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyTagsAction {
	input: ApplyTagsInput,
}

impl LibraryAction for ApplyTagsAction {
	type Input = ApplyTagsInput;
	type Output = ApplyTagsOutput;

	fn from_input(input: ApplyTagsInput) -> Result<Self, String> {
		input.validate()?;
		Ok(Self { input })
	}

	async fn execute(
		self,
		library: Arc<Library>,
		context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let cache = context.ephemeral_cache();
		let device = context
			.device_manager
			.device_id()
			.map_err(|e| ActionError::Internal(format!("no device identity: {e}")))?;
		let stamp = stamp::assertion_stamp(device);

		let (definitions, missing) = definitions::find(&library, &cache, &self.input.tag_ids).await;
		if !missing.is_empty() {
			return Err(ActionError::InvalidInput(format!(
				"unknown tags: {missing:?}"
			)));
		}

		let resolved = targets::resolve(
			&context,
			&self.input.targets,
			&self.input.tag_ids,
			true,
			&stamp,
		)
		.await;
		if resolved.resolved == 0 && resolved.pending == 0 {
			return Err(ActionError::InvalidInput(if resolved.warnings.is_empty() {
				"nothing to tag".to_string()
			} else {
				resolved.warnings.join("; ")
			}));
		}

		// The definition travels with its first assertion, so a store is
		// always able to name the tags it carries.
		for batch in &resolved.batches {
			batch
				.store
				.db()
				.upsert_tag_definitions(&definitions)
				.await
				.map_err(|e| ActionError::Internal(format!("definition write failed: {e}")))?;
			batch
				.store
				.db()
				.append_tag_assertions(&batch.rows)
				.await
				.map_err(|e| ActionError::Internal(format!("assertion write failed: {e}")))?;
		}

		// Remote-owned sources: author the rows durably and deliver when the
		// owner answers. Reachability changes latency, never behavior.
		for batch in &resolved.remote {
			let input = merge::MergeAssertionsInput {
				source_uuid: batch.source_uuid,
				definitions: definitions.iter().map(Into::into).collect(),
				assertions: batch.rows.iter().map(Into::into).collect(),
			};
			outbox::enqueue(
				&library,
				&outbox::RemoteTarget {
					device_uuid: batch.device_uuid,
					source_uuid: batch.source_uuid,
				},
				&input,
			)
			.await
			.map_err(|e| ActionError::Internal(format!("outbox write failed: {e}")))?;
		}
		if !resolved.remote.is_empty() {
			let devices: std::collections::HashSet<uuid::Uuid> =
				resolved.remote.iter().map(|b| b.device_uuid).collect();
			let drain_context = context.clone();
			tokio::spawn(async move {
				for device in devices {
					outbox::drain_for(&drain_context, device).await;
				}
			});
		}

		if let Err(error) = definitions::unstage(&library, &self.input.tag_ids).await {
			tracing::warn!(%error, "applied definitions were not retired from staging");
		}

		crate::domain::File::announce(&context, resolved.affected.clone()).await;

		Ok(ApplyTagsOutput {
			targets_tagged: resolved.resolved,
			targets_pending: resolved.pending,
			warnings: resolved.warnings,
		})
	}

	fn action_kind(&self) -> &'static str {
		"tags.apply"
	}
}

crate::register_library_action!(ApplyTagsAction, "tags.apply");
