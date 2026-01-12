use super::{input::DeleteItemInput, output::DeleteItemOutput};
use crate::{
	context::CoreContext,
	infra::action::{
		error::{ActionError, ActionResult},
		LibraryAction,
	},
};
use sea_orm::{ColumnTrait, EntityTrait, ModelTrait, QueryFilter};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteItemAction {
	input: DeleteItemInput,
}

impl LibraryAction for DeleteItemAction {
	type Input = DeleteItemInput;
	type Output = DeleteItemOutput;

	fn from_input(input: DeleteItemInput) -> Result<Self, String> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		library: std::sync::Arc<crate::library::Library>,
		_context: std::sync::Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let db = library.db().conn();

		let item_model = crate::infra::db::entities::space_item::Entity::find()
			.filter(crate::infra::db::entities::space_item::Column::Uuid.eq(self.input.item_id))
			.one(db)
			.await
			.map_err(ActionError::SeaOrm)?
			.ok_or_else(|| {
				ActionError::Internal(format!("Item {} not found", self.input.item_id))
			})?;

		let item_id = item_model.uuid;

		item_model.delete(db).await.map_err(ActionError::SeaOrm)?;

		// Emit ResourceDeleted event for the item using EventEmitter
		use crate::domain::{resource::EventEmitter, SpaceItem};
		SpaceItem::emit_deleted(item_id, library.event_bus());

		// Emit virtual resource events (space_layout) via ResourceManager
		let resource_manager = crate::domain::ResourceManager::new(
			std::sync::Arc::new(library.db().conn().clone()),
			library.event_bus().clone(),
		);
		resource_manager
			.emit_resource_events("space_item", vec![item_id])
			.await
			.map_err(|e| ActionError::Internal(format!("Failed to emit resource events: {}", e)))?;

		Ok(DeleteItemOutput { success: true })
	}

	fn action_kind(&self) -> &'static str {
		"spaces.delete_item"
	}

	async fn validate(
		&self,
		_library: &std::sync::Arc<crate::library::Library>,
		_context: std::sync::Arc<CoreContext>,
	) -> Result<crate::infra::action::ValidationResult, ActionError> {
		Ok(crate::infra::action::ValidationResult::Success { metadata: None })
	}
}

crate::register_library_action!(DeleteItemAction, "spaces.delete_item");
