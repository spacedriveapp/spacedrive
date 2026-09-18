//! Delete a tag definition.
//!
//! Deletion touches every local home of the tag: live applications get a
//! removal assertion first, so the state change survives on its own terms,
//! then the definition row goes. Assertion history stays; deleting a name is
//! never deleting the claims made under it. A store this daemon cannot write
//! right now keeps its copy, and the definition can return through the
//! adoption path; the tombstone question is recorded in the plan as open.

use super::{input::DeleteTagInput, output::DeleteTagOutput};
use crate::{
	context::CoreContext,
	infra::action::{error::ActionError, LibraryAction},
	library::Library,
	ops::tags::{definitions, stamp},
};
use sd_store::TagAssertion;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteTagAction {
	input: DeleteTagInput,
}

impl LibraryAction for DeleteTagAction {
	type Input = DeleteTagInput;
	type Output = DeleteTagOutput;

	fn from_input(input: DeleteTagInput) -> Result<Self, String> {
		input.validate()?;
		Ok(Self { input })
	}

	async fn execute(
		self,
		library: Arc<Library>,
		context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let cache = context.volume_index();
		let tag_id = self.input.tag_id;

		if definitions::find_one(&library, &cache, tag_id)
			.await
			.is_none()
		{
			return Err(ActionError::InvalidInput(format!("unknown tag: {tag_id}")));
		}

		let device = context
			.device_manager
			.device_id()
			.map_err(|e| ActionError::Internal(format!("no device identity: {e}")))?;
		let stamp = stamp::assertion_stamp(device);

		let mut applications_removed = 0u64;
		let mut sources_updated = 0u32;
		let mut affected = Vec::new();

		for store in cache.stores().await {
			let records = store
				.db()
				.records_with_tag(tag_id)
				.await
				.map_err(|e| ActionError::Internal(format!("store read failed: {e}")))?;

			let mut rows = Vec::with_capacity(records.len());
			for &record in &records {
				let external_id = match cache.path_of_record(record).await {
					Some(path) => store.external_id(&path),
					None => None,
				};
				rows.push(TagAssertion {
					tag_uuid: tag_id,
					record_uuid: record,
					external_id,
					content_uuid: None,
					asserted: false,
					stamp: stamp.clone(),
				});
			}

			applications_removed += store
				.db()
				.append_tag_assertions(&rows)
				.await
				.map_err(|e| ActionError::Internal(format!("assertion write failed: {e}")))?;

			let removed = store
				.db()
				.remove_tag_definition(tag_id)
				.await
				.map_err(|e| ActionError::Internal(format!("definition delete failed: {e}")))?;
			if removed {
				sources_updated += 1;
			}

			affected.extend(records);
		}

		if let Err(error) = definitions::unstage(&library, &[tag_id]).await {
			tracing::warn!(%error, "deleted tag was not removed from staging");
		}

		crate::domain::File::announce(&context, affected).await;

		Ok(DeleteTagOutput {
			applications_removed,
			sources_updated,
		})
	}

	fn action_kind(&self) -> &'static str {
		"tags.delete"
	}
}

crate::register_library_action!(DeleteTagAction, "tags.delete");
