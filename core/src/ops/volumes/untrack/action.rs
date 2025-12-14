//! Volume untrack action

use super::{VolumeUntrackInput, VolumeUntrackOutput};
use crate::{
	context::CoreContext,
	domain::{resource::Identifiable, volume::Volume},
	infra::{action::error::ActionError, db::entities, event::Event},
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeUntrackAction {
	input: VolumeUntrackInput,
}

impl VolumeUntrackAction {
	pub fn new(input: VolumeUntrackInput) -> Self {
		Self { input }
	}
}

crate::register_library_action!(VolumeUntrackAction, "volumes.untrack");

impl crate::infra::action::LibraryAction for VolumeUntrackAction {
	type Input = VolumeUntrackInput;
	type Output = VolumeUntrackOutput;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		Ok(VolumeUntrackAction::new(input))
	}

	async fn execute(
		self,
		library: Arc<crate::library::Library>,
		context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let db = library.db().conn();

		// Find the volume in the database
		let volume = entities::volume::Entity::find()
			.filter(entities::volume::Column::Uuid.eq(self.input.volume_id))
			.one(db)
			.await
			.map_err(|e| ActionError::Internal(e.to_string()))?
			.ok_or_else(|| ActionError::Internal("Volume not found".to_string()))?;

		// Delete the volume from database
		entities::volume::Entity::delete_by_id(volume.id)
			.exec(db)
			.await
			.map_err(|e| ActionError::Internal(e.to_string()))?;

		// Emit ResourceDeleted event using EventEmitter
		use crate::domain::resource::EventEmitter;
		Volume::emit_deleted(self.input.volume_id, &context.events);

		Ok(VolumeUntrackOutput {
			volume_id: self.input.volume_id,
			success: true,
		})
	}

	fn action_kind(&self) -> &'static str {
		"volumes.untrack"
	}
}
