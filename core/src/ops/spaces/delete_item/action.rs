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
			.ok_or_else(|| ActionError::Internal(format!("Item {} not found", self.input.item_id)))?;

		let item_id = item_model.uuid;

		item_model.delete(db).await.map_err(ActionError::SeaOrm)?;

		// Emit ResourceDeleted event for the item
		library.event_bus().emit(crate::infra::event::Event::ResourceDeleted {
			resource_type: "space_item".to_string(),
			resource_id: item_id,
		});

		Ok(DeleteItemOutput { success: true })
	}

	fn action_kind(&self) -> &'static str {
		"spaces.delete_item"
	}

	async fn validate(
		&self,
		_library: std::sync::Arc<crate::library::Library>,
		_context: std::sync::Arc<CoreContext>,
	) -> ActionResult<()> {
		Ok(())
	}
}

crate::register_library_action!(DeleteItemAction, "spaces.delete_item");
