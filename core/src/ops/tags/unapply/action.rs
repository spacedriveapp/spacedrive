//! Remove tags from entries

use super::input::UnapplyTagsInput;
use super::output::UnapplyTagsOutput;
use crate::{
	context::CoreContext,
	infra::action::{error::ActionError, LibraryAction},
	infra::db::entities::{content_identity, entry, tag, user_metadata, user_metadata_tag},
	library::Library,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
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
		_context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let db = library.db();
		let conn = db.conn();

		// Resolve tag UUIDs to database IDs
		let tag_models = tag::Entity::find()
			.filter(tag::Column::Uuid.is_in(self.input.tag_ids.clone()))
			.all(conn)
			.await
			.map_err(|e| ActionError::Internal(format!("Failed to find tags: {}", e)))?;

		let tag_db_ids: Vec<i32> = tag_models.iter().map(|t| t.id).collect();
		if tag_db_ids.is_empty() {
			return Ok(UnapplyTagsOutput {
				entries_affected: 0,
				tags_removed: 0,
				warnings: vec!["No matching tags found".to_string()],
			});
		}

		// Find user_metadata IDs via BOTH entry_uuid and content_identity_uuid
		// (tags can be applied to either, depending on the apply method used)
		let mut um_ids: HashSet<i32> = HashSet::new();

		// 1. Direct match: user_metadata.entry_uuid IN entry_ids
		let um_by_entry = user_metadata::Entity::find()
			.filter(user_metadata::Column::EntryUuid.is_in(self.input.entry_ids.clone()))
			.all(conn)
			.await
			.map_err(|e| ActionError::Internal(format!("DB error: {}", e)))?;
		um_ids.extend(um_by_entry.iter().map(|um| um.id));

		// 2. Indirect match: entry → content_id → content_identity.uuid → user_metadata.content_identity_uuid
		let entries = entry::Entity::find()
			.filter(entry::Column::Uuid.is_in(self.input.entry_ids.clone()))
			.all(conn)
			.await
			.map_err(|e| ActionError::Internal(format!("DB error: {}", e)))?;

		let content_ids: Vec<i32> = entries.iter().filter_map(|e| e.content_id).collect();
		if !content_ids.is_empty() {
			let cis = content_identity::Entity::find()
				.filter(content_identity::Column::Id.is_in(content_ids.clone()))
				.all(conn)
				.await
				.map_err(|e| ActionError::Internal(format!("DB error: {}", e)))?;

			let ci_uuids: Vec<uuid::Uuid> = cis.iter().filter_map(|ci| ci.uuid).collect();
			if !ci_uuids.is_empty() {
				let um_by_content = user_metadata::Entity::find()
					.filter(user_metadata::Column::ContentIdentityUuid.is_in(ci_uuids))
					.all(conn)
					.await
					.map_err(|e| ActionError::Internal(format!("DB error: {}", e)))?;
				um_ids.extend(um_by_content.iter().map(|um| um.id));
			}
		}

		if um_ids.is_empty() {
			return Ok(UnapplyTagsOutput {
				entries_affected: 0,
				tags_removed: 0,
				warnings: vec!["No metadata records found for entries".to_string()],
			});
		}

		// Delete user_metadata_tag records
		let result = user_metadata_tag::Entity::delete_many()
			.filter(user_metadata_tag::Column::UserMetadataId.is_in(um_ids.into_iter().collect::<Vec<_>>()))
			.filter(user_metadata_tag::Column::TagId.is_in(tag_db_ids))
			.exec(conn)
			.await
			.map_err(|e| ActionError::Internal(format!("Failed to remove tags: {}", e)))?;

		let total_removed = result.rows_affected as usize;

		// TODO(sync): Tag unapply is not synced to other devices.
		// The sync infrastructure supports ChangeType::Delete but tag removal
		// does not yet call library.sync_model(). This means removed tags will
		// reappear on other devices after sync. Tracked for a dedicated
		// sync-deletion PR. See also delete/action.rs.

		// Only collect and notify if rows were actually deleted
		if total_removed > 0 {
			// Collect ALL affected entry UUIDs — both directly specified entries
			// and entries that share content with them (content-scoped tags)
			let mut all_affected_uuids: HashSet<uuid::Uuid> =
				self.input.entry_ids.iter().cloned().collect();

			// For content-scoped metadata removal, notify all entries sharing the same content
			if !content_ids.is_empty() {
				let ci_entries = entry::Entity::find()
					.filter(entry::Column::ContentId.is_in(content_ids.into_iter().map(Some)))
					.all(conn)
					.await
					.map_err(|e| ActionError::Internal(format!("DB error: {}", e)))?;
				all_affected_uuids.extend(ci_entries.iter().filter_map(|e| e.uuid));
			}

			// Emit resource events for all affected files
			let resource_manager = crate::domain::ResourceManager::new(
				Arc::new(conn.clone()),
				_context.events.clone(),
			);
			let affected_vec: Vec<uuid::Uuid> = all_affected_uuids.iter().cloned().collect();
			if let Err(e) = resource_manager
				.emit_resource_events("file", affected_vec)
				.await
			{
				tracing::warn!("Failed to emit file resource events after untagging: {}", e);
			}

			Ok(UnapplyTagsOutput {
				entries_affected: all_affected_uuids.len(),
				tags_removed: total_removed,
				warnings: Vec::new(),
			})
		} else {
			Ok(UnapplyTagsOutput {
				entries_affected: 0,
				tags_removed: 0,
				warnings: vec!["No matching tag applications found".to_string()],
			})
		}
	}

	fn action_kind(&self) -> &'static str {
		"tags.unapply"
	}
}

crate::register_library_action!(UnapplyTagsAction, "tags.unapply");
