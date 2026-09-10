//! Apply semantic tags action

use super::{
	input::{ApplyTagsInput, TagTargets},
	output::ApplyTagsOutput,
};
use crate::{
	context::CoreContext,
	domain::tag::{TagApplication, TagSource},
	infra::action::{error::ActionError, LibraryAction},
	library::Library,
	ops::metadata::manager::UserMetadataManager,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyTagsAction {
	input: ApplyTagsInput,
}

impl ApplyTagsAction {
	pub fn new(input: ApplyTagsInput) -> Self {
		Self { input }
	}
}

impl LibraryAction for ApplyTagsAction {
	type Input = ApplyTagsInput;
	type Output = ApplyTagsOutput;

	fn from_input(input: ApplyTagsInput) -> Result<Self, String> {
		input.validate()?;
		Ok(ApplyTagsAction::new(input))
	}

	async fn execute(
		self,
		library: Arc<Library>,
		_context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let db = library.db();
		let cache = _context.ephemeral_cache();
		let metadata_manager = UserMetadataManager::new(Arc::new(db.conn().clone()));
		let device_id = library.id(); // Use library ID as device ID

		let mut warnings = Vec::new();
		let mut successfully_tagged_count = 0;
		let mut missing_target_count = 0;

		// Create tag applications from input
		let tag_applications: Vec<TagApplication> = self
			.input
			.tag_ids
			.iter()
			.map(|&tag_id| {
				let source = self.input.source.clone().unwrap_or(TagSource::User);
				let confidence = self.input.confidence.unwrap_or(1.0);
				let instance_attributes =
					self.input.instance_attributes.clone().unwrap_or_default();

				TagApplication {
					tag_id,
					applied_context: self.input.applied_context.clone(),
					applied_variant: None,
					confidence,
					source,
					instance_attributes,
					created_at: Utc::now(),
					device_uuid: device_id,
				}
			})
			.collect();

		// Collect affected entry UUIDs for resource events
		let mut affected_entry_uuids = Vec::new();

		// Both forms end up on a user_metadata row: one keyed by content, one by
		// the record the volume index minted. The difference is reach, not
		// mechanism.
		match &self.input.targets {
			TagTargets::Content(content_ids) => {
				for &content_id in content_ids {
					match metadata_manager
						.apply_semantic_tags_to_content(
							content_id,
							tag_applications.clone(),
							device_id,
						)
						.await
					{
						Ok(models) => {
							successfully_tagged_count += 1;
							for model in models {
								library
									.sync_model(&model, crate::infra::sync::ChangeType::Insert)
									.await
									.map_err(|e| {
										ActionError::Internal(format!(
											"Failed to sync tag association: {}",
											e
										))
									})?;
							}

							// Every copy of these bytes is now tagged, so every
							// copy has to be told.
							affected_entry_uuids.extend(
								cache
									.copies_of_content(content_id)
									.await
									.into_iter()
									.map(|copy| copy.record_uuid),
							);
						}
						Err(e) => {
							warnings.push(format!("Failed to tag content {}: {}", content_id, e));
						}
					}
				}
			}
			TagTargets::File(record_uuids) => {
				for &record_uuid in record_uuids {
					// A uuid no partition knows is a file that was never walked,
					// which is a different problem from a tag that failed.
					if cache.path_of_record(record_uuid).await.is_none() {
						missing_target_count += 1;
						warnings.push(format!("File {} is not indexed, skipping", record_uuid));
						continue;
					}

					match metadata_manager
						.apply_semantic_tags_to_entry(
							record_uuid,
							tag_applications.clone(),
							device_id,
						)
						.await
					{
						Ok(models) => {
							successfully_tagged_count += 1;
							for model in models {
								library
									.sync_model(&model, crate::infra::sync::ChangeType::Insert)
									.await
									.map_err(|e| {
										ActionError::Internal(format!(
											"Failed to sync tag association: {}",
											e
										))
									})?;
							}
							affected_entry_uuids.push(record_uuid);
						}
						Err(e) => {
							warnings.push(format!("Failed to tag file {}: {}", record_uuid, e));
						}
					}
				}
			}
		}

		// Fail-fast: if NO entries were successfully tagged, return appropriate error.
		if successfully_tagged_count == 0 && !warnings.is_empty() {
			// All failures were missing/unindexed targets (ephemeral files).
			if missing_target_count == warnings.len() {
				return Err(ActionError::InvalidInput(
					"These files need to be indexed before they can be tagged".to_string(),
				));
			}
			// Some or all failures were real execution errors (DB, integrity, etc.).
			return Err(ActionError::Internal(format!(
				"All tag operations failed: {}",
				warnings.join("; ")
			)));
		}

		// Emit resource events for affected files (frontend reactivity)
		if !affected_entry_uuids.is_empty() {
			crate::domain::File::announce(&_context, affected_entry_uuids).await;
		}

		let output = ApplyTagsOutput::success(
			successfully_tagged_count,
			self.input.tag_ids.len(),
			self.input.tag_ids.clone(),
			vec![], // TODO: Return target IDs if needed
		);

		if !warnings.is_empty() {
			Ok(output.with_warnings(warnings))
		} else {
			Ok(output)
		}
	}

	fn action_kind(&self) -> &'static str {
		"tags.apply"
	}
}

// Register library action
crate::register_library_action!(ApplyTagsAction, "tags.apply");
