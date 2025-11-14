//! Location update action handler

use super::output::LocationUpdateOutput;
use crate::{
	context::CoreContext,
	domain::location::JobPolicies,
	infra::action::{
		context::ActionContextProvider,
		error::{ActionError, ActionResult},
		LibraryAction,
	},
	infra::db::entities,
};
use async_trait::async_trait;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use serde_json::json;
use specta::Type;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LocationUpdateInput {
	/// UUID of the location to update
	pub id: Uuid,

	/// Optional new name for the location
	pub name: Option<String>,

	/// Optional job policies to update
	pub job_policies: Option<JobPolicies>,
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
		library: std::sync::Arc<crate::library::Library>,
		context: std::sync::Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let db = library.db().conn();

		// Find the location by UUID
		let location = entities::location::Entity::find()
			.filter(entities::location::Column::Uuid.eq(self.input.id))
			.one(db)
			.await
			.map_err(ActionError::SeaOrm)?
			.ok_or_else(|| ActionError::LocationNotFound(self.input.id))?;

		// Build the update
		let mut active: entities::location::ActiveModel = location.into();

		if let Some(name) = &self.input.name {
			active.name = Set(Some(name.clone()));
		}

		if let Some(ref job_policies) = self.input.job_policies {
			let json_str = serde_json::to_string(job_policies)
				.map_err(|e| ActionError::Internal(format!("Failed to serialize job policies: {}", e)))?;
			active.job_policies = Set(Some(json_str));
		}

		active.updated_at = Set(chrono::Utc::now());

		// Execute update
		active
			.update(db)
			.await
			.map_err(ActionError::SeaOrm)?;

		Ok(LocationUpdateOutput { id: self.input.id })
	}

	fn action_kind(&self) -> &'static str {
		"locations.update"
	}

	async fn validate(
		&self,
		library: std::sync::Arc<crate::library::Library>,
		context: std::sync::Arc<crate::context::CoreContext>,
	) -> Result<(), ActionError> {
		// Validate that the location exists
		let db = library.db().conn();
		let exists = entities::location::Entity::find()
			.filter(entities::location::Column::Uuid.eq(self.input.id))
			.one(db)
			.await
			.map_err(ActionError::SeaOrm)?
			.is_some();

		if !exists {
			return Err(ActionError::LocationNotFound(self.input.id));
		}

		Ok(())
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
