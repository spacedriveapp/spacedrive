//! `spaces.delete`: stop treating a folder as a space.
//!
//! Only the space goes. Its folder, its files and the source that holds them
//! stay, including a workspace's folder, because a space is policy over files
//! and removing the policy must never remove what it covered.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use crate::{
	context::CoreContext,
	infra::action::{error::ActionError, LibraryAction},
	library::Library,
	ops::spaces::store,
};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SpaceDeleteInput {
	pub id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SpaceDeleteOutput {
	pub id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceDeleteAction {
	input: SpaceDeleteInput,
}

impl LibraryAction for SpaceDeleteAction {
	type Input = SpaceDeleteInput;
	type Output = SpaceDeleteOutput;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		library: Arc<Library>,
		context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let space = store::find(&library, &context, self.input.id)
			.await
			.map_err(ActionError::InvalidInput)?;
		let writable = store::writable(&context, &space)
			.await
			.map_err(ActionError::Internal)?;
		let mut row = writable
			.db()
			.space(space.id)
			.await
			.map_err(|e| ActionError::Internal(e.to_string()))?
			.ok_or_else(|| ActionError::InvalidInput(format!("no space {}", space.id)))?;
		row.deleted = true;
		row.set_stamp(&store::stamp(&context).map_err(ActionError::Internal)?);
		writable
			.db()
			.put_space(&row)
			.await
			.map_err(|e| ActionError::Internal(format!("could not delete the space: {e}")))?;
		store::invalidate(space.source_id);
		// Its folder falls back to what the source reads around it: a space
		// that was read may leave it unread, so its text is dropped, and one
		// that was skipped may leave it read, so its files are read.
		store::reread(&library, &context, space.source_id, !space.content_indexing).await?;
		Ok(SpaceDeleteOutput { id: space.id })
	}

	fn action_kind(&self) -> &'static str {
		"spaces.delete"
	}
}

crate::register_library_action!(SpaceDeleteAction, "spaces.delete");
