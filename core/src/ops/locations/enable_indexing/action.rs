use super::output::EnableIndexingOutput;
use crate::{
	context::CoreContext,
	infra::action::{
		context::ActionContextProvider,
		error::{ActionError, ActionResult},
		LibraryAction,
	},
	infra::db::entities,
	location::{manager::LocationManager, IndexMode},
};
use async_trait::async_trait;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use serde_json::json;
use specta::Type;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct EnableIndexingInput {
	/// UUID of the location to enable indexing for
	pub id: Uuid,

	/// Index mode to use (defaults to Deep if not specified)
	#[serde(default = "default_index_mode")]
	pub index_mode: String,
}

fn default_index_mode() -> String {
	"deep".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnableIndexingAction {
	input: EnableIndexingInput,
}

impl EnableIndexingAction {
	pub fn new(input: EnableIndexingInput) -> Self {
		Self { input }
	}
}

impl LibraryAction for EnableIndexingAction {
	type Input = EnableIndexingInput;
	type Output = EnableIndexingOutput;

	fn from_input(input: EnableIndexingInput) -> Result<Self, String> {
		Ok(EnableIndexingAction::new(input))
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

		// Parse the index mode
		let index_mode: IndexMode = self.input.index_mode.as_str().parse().map_err(|e| {
			ActionError::Validation {
				field: "index_mode".to_string(),
				message: format!("Invalid index mode: {}", e),
			}
		})?;

		// Don't allow setting to None
		if index_mode == IndexMode::None {
			return Err(ActionError::Validation {
				field: "index_mode".to_string(),
				message: "Cannot enable indexing with mode 'none'".to_string(),
			});
		}

		// Update the location's index mode
		let mut active: entities::location::ActiveModel = location.clone().into();
		active.index_mode = Set(index_mode.to_string());
		active.updated_at = Set(chrono::Utc::now());

		let updated_location = active.update(db).await.map_err(ActionError::SeaOrm)?;

		// Get the entry and directory path for the location
		let entry_id = updated_location
			.entry_id
			.ok_or_else(|| ActionError::Internal("Location has no entry_id".to_string()))?;

		let directory_path = entities::directory_paths::Entity::find_by_id(entry_id)
			.one(db)
			.await
			.map_err(ActionError::SeaOrm)?
			.ok_or_else(|| {
				ActionError::Internal(format!(
					"No directory path found for location {} entry {}",
					updated_location.uuid, entry_id
				))
			})?;

		// Get device for constructing SdPath
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

		// Construct SdPath
		let sd_path = crate::domain::addressing::SdPath::Physical {
			device_slug: device.slug.clone(),
			path: directory_path.path.clone().into(),
		};

		// Create managed location for indexing
		let managed_location = crate::location::ManagedLocation {
			id: updated_location.uuid,
			name: updated_location
				.name
				.clone()
				.unwrap_or_else(|| "Unknown".to_string()),
			path: directory_path.path.clone().into(),
			device_id: updated_location.device_id,
			library_id: library.id(),
			indexing_enabled: true,
			index_mode,
			watch_enabled: true,
		};

		// Start indexing using LocationManager
		let location_manager = LocationManager::new((*context.events).clone());
		let job_id = location_manager
			.start_indexing_with_context_and_path(
				library.clone(),
				&managed_location,
				sd_path.clone(),
				Some(self.create_action_context()),
			)
			.await
			.map_err(|e| ActionError::Internal(format!("Failed to start indexing: {}", e)))?;

		// Emit ResourceChanged event for UI reactivity
		let job_policies = updated_location
			.job_policies
			.as_ref()
			.and_then(|json| serde_json::from_str(json).ok())
			.unwrap_or_default();

		let location_info = crate::ops::locations::list::LocationInfo {
			id: updated_location.uuid,
			path: directory_path.path.into(),
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

		Ok(EnableIndexingOutput::new(self.input.id, job_id))
	}

	fn action_kind(&self) -> &'static str {
		"locations.enable_indexing"
	}

	async fn validate(
		&self,
		library: &std::sync::Arc<crate::library::Library>,
		context: std::sync::Arc<CoreContext>,
	) -> Result<crate::infra::action::ValidationResult, ActionError> {
		let db = library.db().conn();

		// Validate that the location exists
		let location = entities::location::Entity::find()
			.filter(entities::location::Column::Uuid.eq(self.input.id))
			.one(db)
			.await
			.map_err(ActionError::SeaOrm)?
			.ok_or_else(|| ActionError::LocationNotFound(self.input.id))?;

		// Check if location is already being indexed (not None mode)
		if location.index_mode != "none" {
			return Err(ActionError::Validation {
				field: "id".to_string(),
				message: format!(
					"Location is already indexed with mode '{}'",
					location.index_mode
				),
			});
		}

		Ok(crate::infra::action::ValidationResult::Success)
	}
}

impl ActionContextProvider for EnableIndexingAction {
	fn create_action_context(&self) -> crate::infra::action::context::ActionContext {
		use crate::infra::action::context::{sanitize_action_input, ActionContext};

		ActionContext::new(
			Self::action_type_name(),
			sanitize_action_input(&self.input),
			json!({
				"operation": "enable_indexing",
				"trigger": "user_action",
				"location_id": self.input.id,
			}),
		)
	}

	fn action_type_name() -> &'static str
	where
		Self: Sized,
	{
		"locations.enable_indexing"
	}
}

crate::register_library_action!(EnableIndexingAction, "locations.enable_indexing");
