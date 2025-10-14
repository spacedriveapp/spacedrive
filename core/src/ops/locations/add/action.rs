//! Location add action handler

use super::output::LocationAddOutput;
use crate::{
	context::CoreContext,
	infra::action::{
		context::ActionContextProvider,
		error::{ActionError, ActionResult},
		LibraryAction,
	},
	infra::db::entities,
	location::manager::LocationManager,
	ops::indexing::IndexMode,
};
use async_trait::async_trait;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use serde_json::json;
use specta::Type;
use std::{path::PathBuf, sync::Arc};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LocationAddInput {
	pub path: crate::domain::addressing::SdPath,
	pub name: Option<String>,
	pub mode: IndexMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationAddAction {
	input: LocationAddInput,
}

impl LocationAddAction {
	pub fn new(input: LocationAddInput) -> Self {
		Self { input }
	}
}

// Implement the new modular ActionType trait
impl LibraryAction for LocationAddAction {
	type Input = LocationAddInput;
	type Output = LocationAddOutput;

	fn from_input(input: LocationAddInput) -> Result<Self, String> {
		Ok(LocationAddAction::new(input))
	}

	async fn execute(
		self,
		library: std::sync::Arc<crate::library::Library>,
		context: std::sync::Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		// Get the device UUID from the device manager
		let device_uuid = context
			.device_manager
			.device_id()
			.map_err(ActionError::device_manager_error)?;

		// Get device record from database to get the integer ID
		let db = library.db().conn();
		let device_record = entities::device::Entity::find()
			.filter(entities::device::Column::Uuid.eq(device_uuid))
			.one(db)
			.await
			.map_err(ActionError::SeaOrm)?
			.ok_or_else(|| ActionError::DeviceNotFound(device_uuid))?;

		// Add the location using LocationManager
		let location_manager = LocationManager::new(context.events.as_ref().clone());

		let location_mode = match self.input.mode {
			IndexMode::Shallow => crate::location::IndexMode::Shallow,
			IndexMode::Content => crate::location::IndexMode::Content,
			IndexMode::Deep => crate::location::IndexMode::Deep,
		};

		// Create action context for job tracking
		let action_context = self.create_action_context();

		let (location_id, job_id_string) = location_manager
			.add_location(
				library.clone(),
				self.input.path.clone(),
				self.input.name.clone(),
				device_record.id,
				location_mode,
				Some(action_context),
			)
			.await
			.map_err(|e| ActionError::Internal(e.to_string()))?;

		// Parse the job ID from the string returned by add_location
		let job_id = if !job_id_string.is_empty() {
			Some(
				Uuid::parse_str(&job_id_string)
					.map_err(|e| ActionError::Internal(format!("Failed to parse job ID: {}", e)))?,
			)
		} else {
			None
		};

		let mut output = LocationAddOutput::new(location_id, self.input.path, self.input.name);

		if let Some(job_id) = job_id {
			output = output.with_job_id(job_id);
		}

		Ok(output)
	}

	fn action_kind(&self) -> &'static str {
		"locations.add"
	}

	async fn validate(
		&self,
		library: std::sync::Arc<crate::library::Library>,
		context: std::sync::Arc<crate::context::CoreContext>,
	) -> Result<(), ActionError> {
		use crate::domain::addressing::SdPath;

		match &self.input.path {
			SdPath::Physical { device_id: _, path } => {
				// Validate local filesystem path
				if !path.exists() {
					return Err(ActionError::Validation {
						field: "path".to_string(),
						message: "Path does not exist".to_string(),
					});
				}
				if !path.is_dir() {
					return Err(ActionError::Validation {
						field: "path".to_string(),
						message: "Path must be a directory".to_string(),
					});
				}
			}
			SdPath::Cloud { volume_id, path: cloud_path } => {
				// Validate cloud path
				// Check if the volume exists
				let db = library.db().conn();
				let volume = entities::volume::Entity::find()
					.filter(entities::volume::Column::Uuid.eq(*volume_id))
					.one(db)
					.await
					.map_err(ActionError::SeaOrm)?
					.ok_or_else(|| ActionError::Validation {
						field: "volume_id".to_string(),
						message: format!("Cloud volume {} not found", volume_id),
					})?;

				// TODO: Validate that the path exists on the cloud volume
				// This would require accessing the VolumeBackend, which isn't available in validation
				// For now, we trust the user's input
			}
			SdPath::Content { .. } => {
				return Err(ActionError::Validation {
					field: "path".to_string(),
					message: "Content paths cannot be used as locations".to_string(),
				});
			}
		}

		// Check for duplicate locations
		// TODO: Implement proper duplicate detection for both Physical and Cloud paths

		Ok(())
	}
}

impl ActionContextProvider for LocationAddAction {
	fn create_action_context(&self) -> crate::infra::action::context::ActionContext {
		use crate::infra::action::context::{sanitize_action_input, ActionContext};

		ActionContext::new(
			Self::action_type_name(),
			sanitize_action_input(&self.input),
			json!({
				"operation": "add_location",
				"trigger": "user_action",
				"path": self.input.path.to_string(),
				"name": self.input.name,
				"mode": self.input.mode
			}),
		)
	}

	fn action_type_name() -> &'static str
	where
		Self: Sized,
	{
		"locations.add"
	}
}

// Register action
crate::register_library_action!(LocationAddAction, "locations.add");
