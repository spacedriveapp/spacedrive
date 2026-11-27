use super::{input::LayoutDeleteInput, output::LayoutDeleteOutput};
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
pub struct LayoutDeleteAction {
	input: LayoutDeleteInput,
}

impl LibraryAction for LayoutDeleteAction {
	type Input = LayoutDeleteInput;
	type Output = LayoutDeleteOutput;

	fn from_input(input: LayoutDeleteInput) -> Result<Self, String> {
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

		let layout_id = layout_model.uuid;

		// Delete will cascade to groups and items due to foreign key constraints
		layout_model.delete(db).await.map_err(ActionError::SeaOrm)?;

		// Emit ResourceDeleted event for real-time UI updates using EventEmitter
		use crate::domain::{resource::EventEmitter, Layout};
		Layout::emit_deleted(layout_id, library.event_bus());

		Ok(LayoutDeleteOutput { success: true })
	}

	fn action_kind(&self) -> &'static str {
		"layouts.delete"
	}
}

crate::register_library_action!(LayoutDeleteAction, "layouts.delete");
