use super::{input::UpdateGroupInput, output::UpdateGroupOutput};
use crate::{
	context::CoreContext,
	domain::{GroupType, LayoutGroup},
	infra::action::{
		error::{ActionError, ActionResult},
		LibraryAction,
	},
};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateGroupAction {
	input: UpdateGroupInput,
}

impl LibraryAction for UpdateGroupAction {
	type Input = UpdateGroupInput;
	type Output = UpdateGroupOutput;

	fn from_input(input: UpdateGroupInput) -> Result<Self, String> {
		if let Some(ref name) = input.name {
			if name.trim().is_empty() {
				return Err("Group name cannot be empty".to_string());
			}
		}

		Ok(Self { input })
	}

	async fn execute(
		self,
		library: std::sync::Arc<crate::library::Library>,
		_context: std::sync::Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let db = library.db().conn();

		let group_model = crate::infra::db::entities::layout_group::Entity::find()
			.filter(crate::infra::db::entities::layout_group::Column::Uuid.eq(self.input.group_id))
			.one(db)
			.await
			.map_err(ActionError::SeaOrm)?
			.ok_or_else(|| {
				ActionError::Internal(format!("Group {} not found", self.input.group_id))
			})?;

		let mut active_model: crate::infra::db::entities::layout_group::ActiveModel =
			group_model.clone().into();

		if let Some(name) = self.input.name {
			active_model.name = Set(name);
		}

		if let Some(is_collapsed) = self.input.is_collapsed {
			active_model.is_collapsed = Set(is_collapsed);
		}

		let result = active_model.update(db).await.map_err(ActionError::SeaOrm)?;

		// Sync to peers (emits direct event)
		library
			.sync_model(&result, crate::infra::sync::ChangeType::Update)
			.await
			.map_err(|e| ActionError::Internal(format!("Failed to sync group: {}", e)))?;

		// Emit virtual resource events (sidebar_layout) via ResourceManager
		let resource_manager = crate::domain::ResourceManager::new(
			std::sync::Arc::new(library.db().conn().clone()),
			library.event_bus().clone(),
		);
		resource_manager
			.emit_resource_events("layout_group", vec![result.uuid])
			.await
			.map_err(|e| ActionError::Internal(format!("Failed to emit resource events: {}", e)))?;

		// Parse group_type from JSON
		let group_type: GroupType = serde_json::from_str(&result.group_type)
			.map_err(|e| ActionError::Internal(format!("Failed to parse group_type: {}", e)))?;

		// Get layout UUID
		let layout_model = crate::infra::db::entities::layout::Entity::find_by_id(result.layout_id)
			.one(db)
			.await
			.map_err(ActionError::SeaOrm)?
			.ok_or_else(|| ActionError::Internal("Layout not found for group".to_string()))?;

		let group = LayoutGroup {
			id: result.uuid,
			layout_id: layout_model.uuid,
			name: result.name,
			group_type,
			is_collapsed: result.is_collapsed,
			order: result.order,
			created_at: result.created_at.into(),
		};

		Ok(UpdateGroupOutput { group })
	}

	fn action_kind(&self) -> &'static str {
		"layouts.update_group"
	}
}

crate::register_library_action!(UpdateGroupAction, "layouts.update_group");
