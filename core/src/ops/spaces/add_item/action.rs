use super::{input::AddItemInput, output::AddItemOutput};
use crate::{
	context::CoreContext,
	domain::{addressing::SdPath, ItemType, SpaceItem},
	infra::action::{
		error::{ActionError, ActionResult},
		LibraryAction,
	},
	infra::db::entities::{directory_paths, entry},
};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, NotSet, QueryFilter, QueryOrder, Set};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddItemAction {
	input: AddItemInput,
}

impl LibraryAction for AddItemAction {
	type Input = AddItemInput;
	type Output = AddItemOutput;

	fn from_input(input: AddItemInput) -> Result<Self, String> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		library: std::sync::Arc<crate::library::Library>,
		_context: std::sync::Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let db = library.db().conn();

		// Verify space exists
		let space_model = crate::infra::db::entities::space::Entity::find()
			.filter(crate::infra::db::entities::space::Column::Uuid.eq(self.input.space_id))
			.one(db)
			.await
			.map_err(ActionError::SeaOrm)?
			.ok_or_else(|| {
				ActionError::Internal(format!("Space {} not found", self.input.space_id))
			})?;

		// Verify group exists if group_id is provided
		let group_model_id = if let Some(group_id) = self.input.group_id {
			let group_model = crate::infra::db::entities::space_group::Entity::find()
				.filter(crate::infra::db::entities::space_group::Column::Uuid.eq(group_id))
				.one(db)
				.await
				.map_err(ActionError::SeaOrm)?
				.ok_or_else(|| ActionError::Internal(format!("Group {} not found", group_id)))?;

			Some(group_model.id)
		} else {
			None
		};

		// Get max order (either for space-level or group-level items)
		let max_order = if let Some(group_id) = group_model_id {
			// Max order within group
			crate::infra::db::entities::space_item::Entity::find()
				.filter(crate::infra::db::entities::space_item::Column::GroupId.eq(Some(group_id)))
				.order_by_desc(crate::infra::db::entities::space_item::Column::Order)
				.one(db)
				.await
				.map_err(ActionError::SeaOrm)?
				.map(|i| i.order)
				.unwrap_or(-1)
		} else {
			// Max order for space-level items
			crate::infra::db::entities::space_item::Entity::find()
				.filter(crate::infra::db::entities::space_item::Column::SpaceId.eq(space_model.id))
				.filter(crate::infra::db::entities::space_item::Column::GroupId.is_null())
				.order_by_desc(crate::infra::db::entities::space_item::Column::Order)
				.one(db)
				.await
				.map_err(ActionError::SeaOrm)?
				.map(|i| i.order)
				.unwrap_or(-1)
		};

		let item_id = uuid::Uuid::new_v4();
		let now = Utc::now();

		// Resolve entry_uuid if this is a Path item
		let entry_uuid = if let ItemType::Path { ref sd_path } = self.input.item_type {
			tracing::info!("Resolving SdPath to entry_uuid: {:?}", sd_path);
			let resolved = resolve_sd_path_to_entry_uuid(sd_path, db).await;
			tracing::info!("Resolved entry_uuid: {:?}", resolved);
			resolved
		} else {
			None
		};

		// Serialize item_type to JSON
		let item_type_json = serde_json::to_string(&self.input.item_type)
			.map_err(|e| ActionError::Internal(format!("Failed to serialize item_type: {}", e)))?;

		let active_model = crate::infra::db::entities::space_item::ActiveModel {
			id: NotSet,
			uuid: Set(item_id),
			space_id: Set(space_model.id),
			group_id: Set(group_model_id),
			entry_uuid: Set(entry_uuid),
			item_type: Set(item_type_json),
			order: Set(max_order + 1),
			created_at: Set(now.into()),
		};

		let result = active_model.insert(db).await.map_err(ActionError::SeaOrm)?;

		// Sync to peers (emits direct event)
		library
			.sync_model(&result, crate::infra::sync::ChangeType::Insert)
			.await
			.map_err(|e| ActionError::Internal(format!("Failed to sync item: {}", e)))?;

		// Emit virtual resource events (space_layout) via ResourceManager
		let resource_manager = crate::domain::ResourceManager::new(
			std::sync::Arc::new(library.db().conn().clone()),
			library.event_bus().clone(),
		);
		resource_manager
			.emit_resource_events("space_item", vec![result.uuid])
			.await
			.map_err(|e| ActionError::Internal(format!("Failed to emit resource events: {}", e)))?;

		let item = SpaceItem {
			id: result.uuid,
			space_id: self.input.space_id,
			group_id: self.input.group_id,
			item_type: self.input.item_type,
			order: result.order,
			created_at: result.created_at.into(),
			resolved_file: None, // Not populated in add_item, only in get_layout
		};

		Ok(AddItemOutput { item })
	}

	fn action_kind(&self) -> &'static str {
		"spaces.add_item"
	}

	async fn validate(
		&self,
		_library: &std::sync::Arc<crate::library::Library>,
		_context: std::sync::Arc<CoreContext>,
	) -> Result<crate::infra::action::ValidationResult, ActionError> {
		Ok(crate::infra::action::ValidationResult::Success)
	}
}

crate::register_library_action!(AddItemAction, "spaces.add_item");

/// Resolve an SdPath to an entry UUID by looking up the entry in the database
async fn resolve_sd_path_to_entry_uuid(
	sd_path: &SdPath,
	db: &sea_orm::DatabaseConnection,
) -> Option<uuid::Uuid> {
	match sd_path {
		SdPath::Physical { path, .. } => {
			let path_str = path.to_string_lossy();
			let path_buf = std::path::Path::new(path_str.as_ref());

			let file_name = path_buf.file_name()?.to_string_lossy().to_string();
			let parent_path = path_buf.parent()?.to_string_lossy().to_string();

			tracing::debug!(
				"Looking up entry: file_name={}, parent_path={}",
				file_name,
				parent_path
			);

			// Parse name and extension
			let (name, extension) = if let Some(dot_idx) = file_name.rfind('.') {
				(
					file_name[..dot_idx].to_string(),
					Some(file_name[dot_idx + 1..].to_string()),
				)
			} else {
				(file_name.clone(), None)
			};

			tracing::debug!("Parsed: name={}, extension={:?}", name, extension);

			// Find entry by name/extension
			let mut query = entry::Entity::find().filter(entry::Column::Name.eq(&name));

			if let Some(ext) = &extension {
				query = query.filter(entry::Column::Extension.eq(ext));
			}

			let entries = query.all(db).await.ok()?;
			tracing::debug!("Found {} matching entries by name", entries.len());

			// Find entry with matching parent path
			for e in entries {
				if let Some(parent_id) = e.parent_id {
					if let Ok(Some(parent_path_model)) =
						directory_paths::Entity::find_by_id(parent_id).one(db).await
					{
						tracing::debug!("Entry {} parent path: {}", e.id, parent_path_model.path);
						if parent_path_model.path == parent_path {
							tracing::info!("Matched entry_uuid: {:?}", e.uuid);
							return e.uuid;
						}
					}
				}
			}

			tracing::warn!("No matching entry found for path: {}", path_str);
			None
		}
		_ => {
			tracing::warn!("Non-Physical SdPath not supported for entry resolution");
			None
		}
	}
}
