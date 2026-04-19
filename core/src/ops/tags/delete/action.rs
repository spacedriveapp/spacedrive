//! Delete a tag and all its relationships

use super::input::DeleteTagInput;
use super::output::DeleteTagOutput;
use crate::{
	context::CoreContext,
	infra::action::{error::ActionError, LibraryAction},
	infra::db::entities::{entry, tag, user_metadata, user_metadata_tag},
	library::Library,
	ops::tags::TagManager,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
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
		_context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let db = library.db();
		let conn = db.conn();

		// Collect affected entry UUIDs BEFORE deleting (same pattern as unapply/action.rs)
		let affected_entry_uuids = {
			let tag_model = tag::Entity::find()
				.filter(tag::Column::Uuid.eq(self.input.tag_id))
				.one(conn)
				.await
				.map_err(|e| ActionError::Internal(format!("DB error: {}", e)))?;

			let mut uuids = Vec::new();
			if let Some(tag_model) = tag_model {
				let umt_records = user_metadata_tag::Entity::find()
					.filter(user_metadata_tag::Column::TagId.eq(tag_model.id))
					.all(conn)
					.await
					.map_err(|e| ActionError::Internal(format!("DB error: {}", e)))?;

				let um_ids: Vec<i32> = umt_records.iter().map(|r| r.user_metadata_id).collect();
				if !um_ids.is_empty() {
					let um_records = user_metadata::Entity::find()
						.filter(user_metadata::Column::Id.is_in(um_ids))
						.all(conn)
						.await
						.map_err(|e| ActionError::Internal(format!("DB error: {}", e)))?;

					// Entry-scoped metadata → direct entry UUIDs
					uuids.extend(um_records.iter().filter_map(|um| um.entry_uuid));

					// Content-scoped metadata → find all entries with that content
					let ci_uuids: Vec<uuid::Uuid> = um_records
						.iter()
						.filter_map(|um| um.content_identity_uuid)
						.collect();
					if !ci_uuids.is_empty() {
						let cis = crate::infra::db::entities::content_identity::Entity::find()
							.filter(
								crate::infra::db::entities::content_identity::Column::Uuid
									.is_in(ci_uuids.into_iter().map(Some)),
							)
							.all(conn)
							.await
							.map_err(|e| ActionError::Internal(format!("DB error: {}", e)))?;
						let ci_ids: Vec<i32> = cis.iter().map(|ci| ci.id).collect();
						if !ci_ids.is_empty() {
							let entries = entry::Entity::find()
								.filter(entry::Column::ContentId.is_in(ci_ids.into_iter().map(Some)))
								.all(conn)
								.await
								.map_err(|e| ActionError::Internal(format!("DB error: {}", e)))?;
							uuids.extend(entries.iter().filter_map(|e| e.uuid));
						}
					}
				}
			}
			uuids
		};

		// Delete the tag and all its relationships (atomic transaction)
		let manager = TagManager::new(Arc::new(conn.clone()));
		manager
			.delete_tag(self.input.tag_id)
			.await
			.map_err(|e| ActionError::Internal(format!("Failed to delete tag: {}", e)))?;

		// TODO(sync): Tag deletion is not synced to other devices.
		// The sync infrastructure supports ChangeType::Delete but the tag deletion
		// path does not yet call library.sync_model() with it. This means deleted
		// tags will reappear on other devices after sync. Tracked for a dedicated
		// sync-deletion PR.

		let resource_manager = crate::domain::ResourceManager::new(
			Arc::new(conn.clone()),
			_context.events.clone(),
		);

		// Emit "tag" event so sidebar refreshes
		if let Err(e) = resource_manager
			.emit_resource_events("tag", vec![self.input.tag_id])
			.await
		{
			tracing::warn!("Failed to emit tag resource event after deletion: {}", e);
		}

		// Emit "file" events so explorer grid updates (removes tag dots)
		if !affected_entry_uuids.is_empty() {
			if let Err(e) = resource_manager
				.emit_resource_events("file", affected_entry_uuids)
				.await
			{
				tracing::warn!("Failed to emit file resource events after tag deletion: {}", e);
			}
		}

		Ok(DeleteTagOutput { deleted: true })
	}

	fn action_kind(&self) -> &'static str {
		"tags.delete"
	}
}

crate::register_library_action!(DeleteTagAction, "tags.delete");
