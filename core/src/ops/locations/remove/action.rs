//! Location remove action handler

use super::output::LocationRemoveOutput;
use crate::{
	context::CoreContext,
	infra::action::{error::ActionError, LibraryAction},
	location::manager::LocationManager,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationRemoveInput {
	pub location_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationRemoveAction {
	input: LocationRemoveInput,
}

impl LocationRemoveAction {
	/// Create a new location remove action
	pub fn new(input: LocationRemoveInput) -> Self {
		Self { input }
	}
}

// Implement the unified LibraryAction (replaces ActionHandler)
impl LibraryAction for LocationRemoveAction {
	type Input = LocationRemoveInput;
	type Output = LocationRemoveOutput;

	fn from_input(input: LocationRemoveInput) -> Result<Self, String> {
		Ok(LocationRemoveAction::new(input))
	}

	async fn execute(
		self,
		library: std::sync::Arc<crate::library::Library>,
		context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		// Remove the location
		let location_manager = LocationManager::new(context.events.as_ref().clone());
		location_manager
			.remove_location(&library, self.input.location_id)
			.await
			.map_err(|e| ActionError::Internal(e.to_string()))?;

		Ok(LocationRemoveOutput::new(self.input.location_id, None))
	}

	fn action_kind(&self) -> &'static str {
		"locations.remove"
	}

}

// Register action
crate::register_library_action!(LocationRemoveAction, "locations.remove");
