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
		let mut active: entities::location::ActiveModel = location.clone().into();

		if let Some(name) = &self.input.name {
			active.name = Set(Some(name.clone()));
		}

		if let Some(ref job_policies) = self.input.job_policies {
			let json_str = serde_json::to_string(job_policies).map_err(|e| {
				ActionError::Internal(format!("Failed to serialize job policies: {}", e))
			})?;
			active.job_policies = Set(Some(json_str));
		}

		active.updated_at = Set(chrono::Utc::now());

		// Execute update
		let updated_location = active.update(db).await.map_err(ActionError::SeaOrm)?;

		// Emit ResourceChanged event for UI reactivity
		// Note: job_policies is local-only config (not synced), so we emit regular event not sync event
		// Build LocationInfo for the event
		let entry = entities::entry::Entity::find_by_id(
			updated_location
				.entry_id
				.ok_or_else(|| ActionError::Internal("Location has no entry_id".to_string()))?,
		)
		.one(db)
		.await
		.map_err(ActionError::SeaOrm)?
		.ok_or_else(|| ActionError::Internal("Location entry not found".to_string()))?;

		let directory_path = entities::directory_paths::Entity::find_by_id(entry.id)
			.one(db)
			.await
			.map_err(ActionError::SeaOrm)?
			.ok_or_else(|| {
				ActionError::Internal(format!(
					"No directory path found for location {} entry {}",
					updated_location.uuid, entry.id
				))
			})?;

		let device = entities::device::Entity::find_by_id(updated_location.device_id)
			.one(db)
			.await
			.map_err(ActionError::SeaOrm)?
			.ok_or_else(|| {
				ActionError::Internal(format!(
					"Device not found for location {}",
					updated_location.uuid
				))
			})?;

		let sd_path = crate::domain::SdPath::Physical {
			device_slug: device.slug.clone(),
			path: directory_path.path.clone().into(),
		};

		let job_policies = updated_location
			.job_policies
			.as_ref()
			.and_then(|json| serde_json::from_str(json).ok())
			.unwrap_or_default();

		let location_info = crate::ops::locations::list::LocationInfo {
			id: updated_location.uuid,
			path: directory_path.path.clone().into(),
			name: updated_location.name.clone(),
			sd_path,
			job_policies,
			index_mode: updated_location.index_mode.clone(),
			scan_state: updated_location.scan_state.clone(),
			last_scan_at: updated_location.last_scan_at,
			error_message: updated_location.error_message.clone(),
			total_file_count: updated_location.total_file_count,
			total_byte_size: updated_location.total_byte_size,
			created_at: updated_location.created_at,
			updated_at: updated_location.updated_at,
		};

		context
			.events
			.emit(crate::infra::event::Event::ResourceChanged {
				resource_type: "location".to_string(),
				resource: serde_json::to_value(&location_info).map_err(|e| {
					ActionError::Internal(format!("Failed to serialize location: {}", e))
				})?,
				metadata: None,
			});

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
		let exists = entities::location::Entity::find()
			.filter(entities::location::Column::Uuid.eq(self.input.id))
			.one(db)
			.await
			.map_err(ActionError::SeaOrm)?
			.is_some();

		if !exists {
			return Err(ActionError::LocationNotFound(self.input.id));
		}

		Ok(crate::infra::action::ValidationResult::Success)
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
