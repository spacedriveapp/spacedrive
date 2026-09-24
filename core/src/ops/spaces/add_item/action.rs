use super::{input::AddItemInput, output::AddItemOutput};
use crate::{
	context::CoreContext,
	domain::{addressing::SdPath, ItemType, SpaceItem},
	infra::action::{
		error::{ActionError, ActionResult},
		LibraryAction,
	},
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
		context: std::sync::Arc<CoreContext>,
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

		let entry_uuid = match &self.input.item_type {
			ItemType::Path { sd_path } => {
				resolve_sd_path_to_entry_uuid(sd_path, context.volume_index()).await
			}
			_ => None,
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
}

crate::register_library_action!(AddItemAction, "spaces.add_item");

/// The uuid the volume index gave the file at this path.
///
/// A space item stores it so the pin survives the file being renamed within a
/// directory. `None` for a path outside every source, which pins the path alone.
async fn resolve_sd_path_to_entry_uuid(
	sd_path: &SdPath,
	cache: &crate::ops::indexing::VolumeIndex,
) -> Option<uuid::Uuid> {
	let SdPath::Physical { path, .. } = sd_path else {
		return None;
	};

	cache.ensure_restored(path).await;
	let index = cache.get_for_search(path)?;
	let uuid = index.read().await.get_entry_uuid(path);

	if uuid.is_none() {
		tracing::debug!("No indexed entry for {}", path.display());
	}
	uuid
}
