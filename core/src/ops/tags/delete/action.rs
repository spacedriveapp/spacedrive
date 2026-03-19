//! Delete a tag and all its relationships

use super::input::DeleteTagInput;
use super::output::DeleteTagOutput;
use crate::{
	context::CoreContext,
	infra::action::{error::ActionError, LibraryAction},
	library::Library,
	ops::tags::TagManager,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteTagAction {
	input: DeleteTagInput,
}

impl LibraryAction for DeleteTagAction {
	type Input = DeleteTagInput;
	type Output = DeleteTagOutput;

	fn from_input(input: DeleteTagInput) -> Result<Self, String> {
		input.validate()?;
		Ok(Self { input })
	}

	async fn execute(
		self,
		library: Arc<Library>,
		_context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let db = library.db();
		let manager = TagManager::new(Arc::new(db.conn().clone()));

		manager
			.delete_tag(self.input.tag_id)
			.await
			.map_err(|e| ActionError::Internal(format!("Failed to delete tag: {}", e)))?;

		// TODO: call sync_model with ChangeType::Delete for cross-device sync
		// (not yet implemented in the sync system for deletions)

		// Emit resource event so frontend refreshes tag lists
		let resource_manager = crate::domain::ResourceManager::new(
			Arc::new(db.conn().clone()),
			_context.events.clone(),
		);
		if let Err(e) = resource_manager
			.emit_resource_events("tag", vec![self.input.tag_id])
			.await
		{
			tracing::warn!("Failed to emit tag resource event after deletion: {}", e);
		}

		Ok(DeleteTagOutput { deleted: true })
	}

	fn action_kind(&self) -> &'static str {
		"tags.delete"
	}
}

crate::register_library_action!(DeleteTagAction, "tags.delete");
