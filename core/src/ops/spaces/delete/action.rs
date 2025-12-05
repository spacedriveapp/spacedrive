use super::{input::SpaceDeleteInput, output::SpaceDeleteOutput};
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
pub struct SpaceDeleteAction {
	input: SpaceDeleteInput,
}

impl LibraryAction for SpaceDeleteAction {
	type Input = SpaceDeleteInput;
	type Output = SpaceDeleteOutput;

	fn from_input(input: SpaceDeleteInput) -> Result<Self, String> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		library: std::sync::Arc<crate::library::Library>,
		_context: std::sync::Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let db = library.db().conn();

		let space_model = crate::infra::db::entities::space::Entity::find()
			.filter(crate::infra::db::entities::space::Column::Uuid.eq(self.input.space_id))
			.one(db)
			.await
			.map_err(ActionError::SeaOrm)?
			.ok_or_else(|| {
				ActionError::Internal(format!("Space {} not found", self.input.space_id))
			})?;

		let space_id = space_model.uuid;

		// Delete will cascade to groups and items due to foreign key constraints
		space_model.delete(db).await.map_err(ActionError::SeaOrm)?;

		// Emit ResourceDeleted event for real-time UI updates
		library
			.event_bus()
			.emit(crate::infra::event::Event::ResourceDeleted {
				resource_type: "space".to_string(),
				resource_id: space_id,
			});

		Ok(SpaceDeleteOutput { success: true })
	}

	fn action_kind(&self) -> &'static str {
		"spaces.delete"
	}

	async fn validate(
		&self,
		_library: &std::sync::Arc<crate::library::Library>,
		_context: std::sync::Arc<CoreContext>,
	) -> Result<crate::infra::action::ValidationResult, ActionError> {
		Ok(crate::infra::action::ValidationResult::Success)
	}
}

crate::register_library_action!(SpaceDeleteAction, "spaces.delete");
