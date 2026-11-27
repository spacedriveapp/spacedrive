//! `spaces.update`: rename a space, turn its content indexing on or off, or
//! replace its protected patterns.

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
	/// The space's protected patterns, replacing the ones it has. An empty
	/// list leaves only the built-in protection.
	pub protected: Option<Vec<String>>,
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
		if let Some(patterns) = &input.protected {
			store::protected_patterns(patterns.clone())?;
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
		let reading = self
			.input
			.content_indexing
			.filter(|reading| *reading != row.content_indexing);
		if let Some(reading) = reading {
			row.content_indexing = reading;
		}
		if let Some(patterns) = self.input.protected {
			row.protected =
				store::protected_patterns(patterns).map_err(ActionError::InvalidInput)?;
		}
		row.set_stamp(&store::stamp(&context).map_err(ActionError::Internal)?);
		writable
			.db()
			.put_space(&row)
			.await
			.map_err(|e| ActionError::Internal(format!("could not save the space: {e}")))?;
		store::invalidate(space.source_id);
		if let Some(reading) = reading {
			store::reread(&library, &context, space.source_id, reading).await?;
		}

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
