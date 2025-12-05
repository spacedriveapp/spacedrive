use super::{
	input::{ReorderGroupsInput, ReorderItemsInput},
	output::ReorderOutput,
};
use crate::{
	context::CoreContext,
	infra::action::{
		error::{ActionError, ActionResult},
		LibraryAction,
	},
};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReorderGroupsAction {
	input: ReorderGroupsInput,
}

impl LibraryAction for ReorderGroupsAction {
	type Input = ReorderGroupsInput;
	type Output = ReorderOutput;

	fn from_input(input: ReorderGroupsInput) -> Result<Self, String> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		library: std::sync::Arc<crate::library::Library>,
		_context: std::sync::Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let db = library.db().conn();

		// Update order for each group
		for (index, group_id) in self.input.group_ids.iter().enumerate() {
			let group_model = crate::infra::db::entities::space_group::Entity::find()
				.filter(crate::infra::db::entities::space_group::Column::Uuid.eq(*group_id))
				.one(db)
				.await
				.map_err(ActionError::SeaOrm)?
				.ok_or_else(|| ActionError::Internal(format!("Group {} not found", group_id)))?;

			let mut active_model: crate::infra::db::entities::space_group::ActiveModel =
				group_model.into();
			active_model.order = Set(index as i32);
			active_model.update(db).await.map_err(ActionError::SeaOrm)?;
		}

		Ok(ReorderOutput { success: true })
	}

	fn action_kind(&self) -> &'static str {
		"spaces.reorder_groups"
	}

	async fn validate(
		&self,
		_library: &std::sync::Arc<crate::library::Library>,
		_context: std::sync::Arc<CoreContext>,
	) -> Result<crate::infra::action::ValidationResult, ActionError> {
		Ok(crate::infra::action::ValidationResult::Success)
	}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReorderItemsAction {
	input: ReorderItemsInput,
}

impl LibraryAction for ReorderItemsAction {
	type Input = ReorderItemsInput;
	type Output = ReorderOutput;

	fn from_input(input: ReorderItemsInput) -> Result<Self, String> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		library: std::sync::Arc<crate::library::Library>,
		_context: std::sync::Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let db = library.db().conn();

		// Update order for each item
		for (index, item_id) in self.input.item_ids.iter().enumerate() {
			let item_model = crate::infra::db::entities::space_item::Entity::find()
				.filter(crate::infra::db::entities::space_item::Column::Uuid.eq(*item_id))
				.one(db)
				.await
				.map_err(ActionError::SeaOrm)?
				.ok_or_else(|| ActionError::Internal(format!("Item {} not found", item_id)))?;

			let mut active_model: crate::infra::db::entities::space_item::ActiveModel =
				item_model.into();
			active_model.order = Set(index as i32);
			active_model.update(db).await.map_err(ActionError::SeaOrm)?;
		}

		Ok(ReorderOutput { success: true })
	}

	fn action_kind(&self) -> &'static str {
		"spaces.reorder_items"
	}

	async fn validate(
		&self,
		_library: &std::sync::Arc<crate::library::Library>,
		_context: std::sync::Arc<CoreContext>,
	) -> Result<crate::infra::action::ValidationResult, ActionError> {
		Ok(crate::infra::action::ValidationResult::Success)
	}
}

crate::register_library_action!(ReorderGroupsAction, "spaces.reorder_groups");
crate::register_library_action!(ReorderItemsAction, "spaces.reorder_items");
