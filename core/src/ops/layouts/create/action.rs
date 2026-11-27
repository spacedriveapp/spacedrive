use super::{input::LayoutCreateInput, output::LayoutCreateOutput};
use crate::{
	context::CoreContext,
	domain::Layout,
	infra::action::{
		error::{ActionError, ActionResult},
		LibraryAction,
	},
};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, EntityTrait, QueryOrder, Set};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutCreateAction {
	input: LayoutCreateInput,
}

impl LayoutCreateAction {
	pub fn new(input: LayoutCreateInput) -> Self {
		Self { input }
	}
}

impl LibraryAction for LayoutCreateAction {
	type Input = LayoutCreateInput;
	type Output = LayoutCreateOutput;

	fn from_input(input: LayoutCreateInput) -> Result<Self, String> {
		// Validate input
		if input.name.trim().is_empty() {
			return Err("Layout name cannot be empty".to_string());
		}

		if !Layout::validate_color(&input.color) {
			return Err("Invalid color format. Must be #RRGGBB".to_string());
		}

		Ok(LayoutCreateAction::new(input))
	}

	async fn execute(
		self,
		library: std::sync::Arc<crate::library::Library>,
		_context: std::sync::Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let db = library.db().conn();

		// Get the current max order
		let max_order = crate::infra::db::entities::layout::Entity::find()
			.order_by_desc(crate::infra::db::entities::layout::Column::Order)
			.one(db)
			.await
			.map_err(ActionError::SeaOrm)?
			.map(|s| s.order)
			.unwrap_or(-1);

		let layout_id = uuid::Uuid::new_v4();
		let now = Utc::now();

		// Create layout entity
		let active_model = crate::infra::db::entities::layout::ActiveModel {
			id: sea_orm::NotSet, // Auto-increment
			uuid: Set(layout_id),
			name: Set(self.input.name.clone()),
			icon: Set(self.input.icon.clone()),
			color: Set(self.input.color.clone()),
			order: Set(max_order + 1),
			created_at: Set(now.into()),
			updated_at: Set(now.into()),
		};

		let result = active_model.insert(db).await.map_err(ActionError::SeaOrm)?;

		// Sync to peers (emits direct event)
		library
			.sync_model(&result, crate::infra::sync::ChangeType::Insert)
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

		Ok(LayoutCreateOutput { layout })
	}

	fn action_kind(&self) -> &'static str {
		"layouts.create"
	}
}

crate::register_library_action!(LayoutCreateAction, "layouts.create");
