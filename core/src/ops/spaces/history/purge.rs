//! `spaces.history.purge`: delete every version and snapshot a space keeps.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use crate::{
	context::CoreContext,
	infra::action::{error::ActionError, LibraryAction},
	library::Library,
	ops::spaces::{store, versioning},
};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SpaceHistoryPurgeInput {
	pub id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SpaceHistoryPurgeOutput {
	/// Whether the space had versions to delete.
	pub purged: bool,
}

/// Turning versioning off keeps what was stored, so freeing that disk space
/// is this separate, explicit step. The space's change log stays.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceHistoryPurgeAction {
	input: SpaceHistoryPurgeInput,
}

impl LibraryAction for SpaceHistoryPurgeAction {
	type Input = SpaceHistoryPurgeInput;
	type Output = SpaceHistoryPurgeOutput;

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
		let purged = versioning::purge(&context, &space)
			.await
			.map_err(ActionError::Internal)?;
		Ok(SpaceHistoryPurgeOutput { purged })
	}

	fn action_kind(&self) -> &'static str {
		"spaces.history.purge"
	}
}

crate::register_library_action!(SpaceHistoryPurgeAction, "spaces.history.purge");
