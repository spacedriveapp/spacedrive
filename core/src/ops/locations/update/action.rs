//! Location update action handler

use super::output::LocationUpdateOutput;
use crate::{
	context::CoreContext,
	infra::action::{
		context::ActionContextProvider,
		error::{ActionError, ActionResult},
		LibraryAction,
	},
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;
use specta::Type;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LocationUpdateInput {
	/// UUID of the location to update
	pub id: Uuid,

	/// A new name. The path is not editable: a pin somewhere else is a
	/// different pin.
	pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationUpdateAction {
	input: LocationUpdateInput,
}

impl LocationUpdateAction {
	pub fn new(input: LocationUpdateInput) -> Self {
		Self { input }
	}
}

impl LibraryAction for LocationUpdateAction {
	type Input = LocationUpdateInput;
	type Output = LocationUpdateOutput;

	fn from_input(input: LocationUpdateInput) -> Result<Self, String> {
		Ok(LocationUpdateAction::new(input))
	}

	async fn execute(
		self,
		library: Arc<crate::library::Library>,
		_context: Arc<CoreContext>,
	) -> ActionResult<Self::Output> {
		if let Some(name) = self.input.name.clone() {
			crate::location::rename(&library, self.input.id, name)
				.await
				.map_err(|e| ActionError::Internal(e.to_string()))?;
		}

		Ok(LocationUpdateOutput { id: self.input.id })
	}

	fn action_kind(&self) -> &'static str {
		"locations.update"
	}

	async fn validate(
		&self,
		library: &std::sync::Arc<crate::library::Library>,
		context: std::sync::Arc<crate::context::CoreContext>,
	) -> Result<crate::infra::action::ValidationResult, ActionError> {
		// Validate that the location exists
		let db = library.db().conn();
		use crate::infra::db::entities::location;
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

		let exists = location::Entity::find()
			.filter(location::Column::Uuid.eq(self.input.id))
			.one(db)
			.await
			.map_err(ActionError::SeaOrm)?
			.is_some();

		if !exists {
			return Err(ActionError::LocationNotFound(self.input.id));
		}

		Ok(crate::infra::action::ValidationResult::Success { metadata: None })
	}
}

impl ActionContextProvider for LocationUpdateAction {
	fn create_action_context(&self) -> crate::infra::action::context::ActionContext {
		use crate::infra::action::context::{sanitize_action_input, ActionContext};

		ActionContext::new(
			Self::action_type_name(),
			sanitize_action_input(&self.input),
			json!({
				"operation": "update_location",
				"trigger": "user_action",
				"location_id": self.input.id,
			}),
		)
	}

	fn action_type_name() -> &'static str
	where
		Self: Sized,
	{
		"locations.update"
	}
}

// Register action
crate::register_library_action!(LocationUpdateAction, "locations.update");
