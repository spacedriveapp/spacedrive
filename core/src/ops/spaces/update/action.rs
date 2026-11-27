//! `spaces.update`: rename a space or turn its content indexing on or off.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use crate::{
	context::CoreContext,
	domain::Space,
	infra::action::{error::ActionError, LibraryAction},
	library::Library,
	ops::spaces::store,
};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SpaceUpdateInput {
	pub id: Uuid,
	pub name: Option<String>,
	pub content_indexing: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SpaceUpdateOutput {
	pub space: Space,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceUpdateAction {
	input: SpaceUpdateInput,
}

impl LibraryAction for SpaceUpdateAction {
	type Input = SpaceUpdateInput;
	type Output = SpaceUpdateOutput;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		if input
			.name
			.as_ref()
			.is_some_and(|name| name.trim().is_empty())
		{
			return Err("the name cannot be empty".to_string());
		}
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
		if let Some(name) = self.input.name {
			row.name = name.trim().to_string();
		}
		if let Some(content_indexing) = self.input.content_indexing {
			row.content_indexing = content_indexing;
		}
		row.set_stamp(&store::stamp(&context).map_err(ActionError::Internal)?);
		writable
			.db()
			.put_space(&row)
			.await
			.map_err(|e| ActionError::Internal(format!("could not save the space: {e}")))?;

		let root = context.volume_index().source_root(space.source_id);
		Ok(SpaceUpdateOutput {
			space: Space::from_row(row, space.source_id, root.as_deref()),
		})
	}

	fn action_kind(&self) -> &'static str {
		"spaces.update"
	}
}

crate::register_library_action!(SpaceUpdateAction, "spaces.update");
