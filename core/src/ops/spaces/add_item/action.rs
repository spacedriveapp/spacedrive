use super::{input::AddItemInput, output::AddItemOutput};
use crate::{
	context::CoreContext,
	domain::SpaceItem,
	infra::action::{
		error::{ActionError, ActionResult},
		LibraryAction,
	},
};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
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

		// Verify group exists
		let group_model = crate::infra::db::entities::space_group::Entity::find()
			.filter(crate::infra::db::entities::space_group::Column::Uuid.eq(self.input.group_id))
			.one(db)
			.await
			.map_err(ActionError::SeaOrm)?
			.ok_or_else(|| ActionError::Internal(format!("Group {} not found", self.input.group_id)))?;

		// Get max order for this group
		let max_order = crate::infra::db::entities::space_item::Entity::find()
			.filter(crate::infra::db::entities::space_item::Column::GroupId.eq(group_model.id))
			.order_by_desc(crate::infra::db::entities::space_item::Column::Order)
			.one(db)
			.await
			.map_err(ActionError::SeaOrm)?
			.map(|i| i.order)
			.unwrap_or(-1);

		let item_id = uuid::Uuid::new_v4();
		let now = Utc::now();

		// Serialize item_type to JSON
		let item_type_json = serde_json::to_string(&self.input.item_type)
			.map_err(|e| ActionError::Internal(format!("Failed to serialize item_type: {}", e)))?;

		let active_model = crate::infra::db::entities::space_item::ActiveModel {
			id: Set(0),
			uuid: Set(item_id),
			group_id: Set(group_model.id),
			item_type: Set(item_type_json),
			order: Set(max_order + 1),
			created_at: Set(now.into()),
		};

		let result = active_model.insert(db).await.map_err(ActionError::SeaOrm)?;

		let item = SpaceItem {
			id: result.uuid,
			group_id: self.input.group_id,
			item_type: self.input.item_type,
			order: result.order,
			created_at: result.created_at.into(),
		};

		Ok(AddItemOutput { item })
	}

	fn action_kind(&self) -> &'static str {
		"spaces.add_item"
	}

	async fn validate(
		&self,
		_library: std::sync::Arc<crate::library::Library>,
		_context: std::sync::Arc<CoreContext>,
	) -> ActionResult<()> {
		Ok(())
	}
}

crate::register_library_action!(AddItemAction, "spaces.add_item");
