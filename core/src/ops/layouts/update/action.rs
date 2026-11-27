use super::{input::LayoutUpdateInput, output::LayoutUpdateOutput};
use crate::{
	context::CoreContext,
	domain::Layout,
	infra::action::{
		error::{ActionError, ActionResult},
		LibraryAction,
	},
};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutUpdateAction {
	input: LayoutUpdateInput,
}

impl LibraryAction for LayoutUpdateAction {
	type Input = LayoutUpdateInput;
	type Output = LayoutUpdateOutput;

	fn from_input(input: LayoutUpdateInput) -> Result<Self, String> {
		if let Some(ref name) = input.name {
			if name.trim().is_empty() {
				return Err("Layout name cannot be empty".to_string());
			}
		}

		if let Some(ref color) = input.color {
			if !Layout::validate_color(color) {
				return Err("Invalid color format. Must be #RRGGBB".to_string());
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

		let layout_model = crate::infra::db::entities::layout::Entity::find()
			.filter(crate::infra::db::entities::layout::Column::Uuid.eq(self.input.layout_id))
			.one(db)
			.await
			.map_err(ActionError::SeaOrm)?
			.ok_or_else(|| {
				ActionError::Internal(format!("Layout {} not found", self.input.layout_id))
			})?;

		let mut active_model: crate::infra::db::entities::layout::ActiveModel = layout_model.into();

		if let Some(name) = self.input.name {
			active_model.name = Set(name);
		}

		if let Some(icon) = self.input.icon {
			active_model.icon = Set(icon);
		}

		if let Some(color) = self.input.color {
			active_model.color = Set(color);
		}

		active_model.updated_at = Set(Utc::now().into());

		let result = active_model.update(db).await.map_err(ActionError::SeaOrm)?;

		// Sync to peers (emits direct event)
		library
			.sync_model(&result, crate::infra::sync::ChangeType::Update)
			.await
			.map_err(|e| ActionError::Internal(format!("Failed to sync layout: {}", e)))?;

		// Emit virtual resource events (sidebar_layout) via ResourceManager
		let resource_manager = crate::domain::ResourceManager::new(
			std::sync::Arc::new(library.db().conn().clone()),
			library.event_bus().clone(),
		);
		resource_manager
			.emit_resource_events("layout", vec![result.uuid])
			.await
			.map_err(|e| ActionError::Internal(format!("Failed to emit resource events: {}", e)))?;

		let layout = Layout {
			id: result.uuid,
			name: result.name,
			icon: result.icon,
			color: result.color,
			order: result.order,
			created_at: result.created_at.into(),
			updated_at: result.updated_at.into(),
		};

		Ok(LayoutUpdateOutput { layout })
	}

	fn action_kind(&self) -> &'static str {
		"layouts.update"
	}
}

crate::register_library_action!(LayoutUpdateAction, "layouts.update");
