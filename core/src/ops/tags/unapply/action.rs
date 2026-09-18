//! Remove tags: append removal assertions.
//!
//! Removal is a row with `asserted = 0`, never a delete, so a store that was
//! away when the removal happened cannot resurrect the tag when it returns.

use super::{input::UnapplyTagsInput, output::UnapplyTagsOutput};
use crate::{
	context::CoreContext,
	infra::action::{error::ActionError, LibraryAction},
	library::Library,
	ops::tags::{definitions, merge, outbox, stamp, targets},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnapplyTagsAction {
	input: UnapplyTagsInput,
}

impl LibraryAction for UnapplyTagsAction {
	type Input = UnapplyTagsInput;
	type Output = UnapplyTagsOutput;

	fn from_input(input: UnapplyTagsInput) -> Result<Self, String> {
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

		// Validate the names, though a removal row does not need the
		// definition present to be correct.
		let (_, missing) = definitions::find(&library, &cache, &self.input.tag_ids).await;
		if !missing.is_empty() {
			return Err(ActionError::InvalidInput(format!(
				"unknown tags: {missing:?}"
			)));
		}

		let resolved = targets::resolve(
			&context,
			&self.input.targets,
			&self.input.tag_ids,
			false,
			&stamp,
		)
		.await;
		if resolved.resolved == 0 && resolved.pending == 0 {
			return Err(ActionError::InvalidInput(if resolved.warnings.is_empty() {
				"nothing to untag".to_string()
			} else {
				resolved.warnings.join("; ")
			}));
		}

		for batch in &resolved.batches {
			batch
				.store
				.db()
				.append_tag_assertions(&batch.rows)
				.await
				.map_err(|e| ActionError::Internal(format!("assertion write failed: {e}")))?;
		}

		// A removal for a remote-owned source rides the same outbox; the
		// definitions list stays empty because a removal needs no name.
		for batch in &resolved.remote {
			let input = merge::MergeAssertionsInput {
				source_uuid: batch.source_uuid,
				definitions: Vec::new(),
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

		crate::domain::File::announce(&context, resolved.affected.clone()).await;

		Ok(UnapplyTagsOutput {
			targets_untagged: resolved.resolved,
			targets_pending: resolved.pending,
			warnings: resolved.warnings,
		})
	}

	fn action_kind(&self) -> &'static str {
		"tags.unapply"
	}
}

crate::register_library_action!(UnapplyTagsAction, "tags.unapply");
