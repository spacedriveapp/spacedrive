use super::{input::AddGroupInput, output::AddGroupOutput};
use crate::{
	context::CoreContext,
	domain::SpaceGroup,
	infra::action::{
		error::{ActionError, ActionResult},
		LibraryAction,
	},
};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddGroupAction {
	input: AddGroupInput,
}

impl LibraryAction for AddGroupAction {
	type Input = AddGroupInput;
	type Output = AddGroupOutput;

	fn from_input(input: AddGroupInput) -> Result<Self, String> {
		if input.name.trim().is_empty() {
			return Err("Group name cannot be empty".to_string());
		}

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
			.ok_or_else(|| ActionError::Internal(format!("Space {} not found", self.input.space_id)))?;

		// Get max order for this space
		let max_order = crate::infra::db::entities::space_group::Entity::find()
			.filter(crate::infra::db::entities::space_group::Column::SpaceId.eq(space_model.id))
			.order_by_desc(crate::infra::db::entities::space_group::Column::Order)
			.one(db)
			.await
			.map_err(ActionError::SeaOrm)?
			.map(|g| g.order)
			.unwrap_or(-1);

		let group_id = uuid::Uuid::new_v4();
		let now = Utc::now();

		// Serialize group_type to JSON
		let group_type_json = serde_json::to_string(&self.input.group_type)
			.map_err(|e| ActionError::Internal(format!("Failed to serialize group_type: {}", e)))?;

		let active_model = crate::infra::db::entities::space_group::ActiveModel {
			id: Set(0),
			uuid: Set(group_id),
			space_id: Set(space_model.id),
			name: Set(self.input.name.clone()),
			group_type: Set(group_type_json),
			is_collapsed: Set(false),
			order: Set(max_order + 1),
			created_at: Set(now.into()),
		};

		let result = active_model.insert(db).await.map_err(ActionError::SeaOrm)?;

		let group = SpaceGroup {
			id: result.uuid,
			space_id: self.input.space_id,
			name: result.name,
			group_type: self.input.group_type,
			is_collapsed: result.is_collapsed,
			order: result.order,
			created_at: result.created_at.into(),
		};

		Ok(AddGroupOutput { group })
	}

	fn action_kind(&self) -> &'static str {
		"spaces.add_group"
	}

	async fn validate(
		&self,
		_library: std::sync::Arc<crate::library::Library>,
		_context: std::sync::Arc<CoreContext>,
	) -> ActionResult<()> {
		Ok(())
	}
}

crate::register_library_action!(AddGroupAction, "spaces.add_group");
