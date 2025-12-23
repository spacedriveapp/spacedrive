use super::{input::DeleteGroupInput, output::DeleteGroupOutput};
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
pub struct DeleteGroupAction {
	input: DeleteGroupInput,
}

impl LibraryAction for DeleteGroupAction {
	type Input = DeleteGroupInput;
	type Output = DeleteGroupOutput;

	fn from_input(input: DeleteGroupInput) -> Result<Self, String> {
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
			.ok_or_else(|| {
				ActionError::Internal(format!("Group {} not found", self.input.group_id))
			})?;

		let group_id = group_model.uuid;

		// Delete will cascade to items due to foreign key constraints
		group_model.delete(db).await.map_err(ActionError::SeaOrm)?;

		// Emit ResourceDeleted event for the group using EventEmitter
		use crate::domain::{resource::EventEmitter, SpaceGroup};
		SpaceGroup::emit_deleted(group_id, library.event_bus());

		// Emit virtual resource events (space_layout) via ResourceManager
		let resource_manager = crate::domain::ResourceManager::new(
			std::sync::Arc::new(library.db().conn().clone()),
			library.event_bus().clone(),
		);
		resource_manager
			.emit_resource_events("space_group", vec![group_id])
			.await
			.map_err(|e| ActionError::Internal(format!("Failed to emit resource events: {}", e)))?;

		Ok(DeleteGroupOutput { success: true })
	}

	fn action_kind(&self) -> &'static str {
		"spaces.delete_group"
	}

	async fn validate(
		&self,
		_library: &std::sync::Arc<crate::library::Library>,
		_context: std::sync::Arc<CoreContext>,
	) -> Result<crate::infra::action::ValidationResult, ActionError> {
		Ok(crate::infra::action::ValidationResult::Success)
	}
}

crate::register_library_action!(DeleteGroupAction, "spaces.delete_group");
