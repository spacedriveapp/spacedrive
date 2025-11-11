use super::{input::UpdateGroupInput, output::UpdateGroupOutput};
use crate::{
	context::CoreContext,
	domain::{GroupType, SpaceGroup},
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

		let group_model = crate::infra::db::entities::space_group::Entity::find()
			.filter(crate::infra::db::entities::space_group::Column::Uuid.eq(self.input.group_id))
			.one(db)
			.await
			.map_err(ActionError::SeaOrm)?
			.ok_or_else(|| ActionError::Internal(format!("Group {} not found", self.input.group_id)))?;

		let mut active_model: crate::infra::db::entities::space_group::ActiveModel = group_model.clone().into();

		if let Some(name) = self.input.name {
			active_model.name = Set(name);
		}

		if let Some(is_collapsed) = self.input.is_collapsed {
			active_model.is_collapsed = Set(is_collapsed);
		}

		let result = active_model.update(db).await.map_err(ActionError::SeaOrm)?;

		// Parse group_type from JSON
		let group_type: GroupType = serde_json::from_str(&result.group_type)
			.map_err(|e| ActionError::Internal(format!("Failed to parse group_type: {}", e)))?;

		// Get space UUID
		let space_model = crate::infra::db::entities::space::Entity::find_by_id(result.space_id)
			.one(db)
			.await
			.map_err(ActionError::SeaOrm)?
			.ok_or_else(|| ActionError::Internal("Space not found for group".to_string()))?;

		let group = SpaceGroup {
			id: result.uuid,
			space_id: space_model.uuid,
			name: result.name,
			group_type,
			is_collapsed: result.is_collapsed,
			order: result.order,
			created_at: result.created_at.into(),
		};

		Ok(UpdateGroupOutput { group })
	}

	fn action_kind(&self) -> &'static str {
		"spaces.update_group"
	}

	async fn validate(
		&self,
		_library: std::sync::Arc<crate::library::Library>,
		_context: std::sync::Arc<CoreContext>,
	) -> ActionResult<()> {
		Ok(())
	}
}

crate::register_library_action!(UpdateGroupAction, "spaces.update_group");
