//! Location add action handler

use super::output::LocationAddOutput;
use crate::{
	context::CoreContext,
	infra::action::{
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
use std::{path::PathBuf, sync::Arc};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationAddInput {
	pub library_id: Uuid,
	pub path: PathBuf,
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

		let (location_id, job_id_string) = location_manager
			.add_location(
				library.clone(),
				self.input.path.clone(),
				self.input.name.clone(),
				device_record.id,
				location_mode,
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

		Ok(LocationAddOutput::new(
			location_id,
			self.input.path,
			self.input.name,
		))
	}

	fn action_kind(&self) -> &'static str {
		"locations.add"
	}

	async fn validate(
		&self,
		library: &std::sync::Arc<crate::library::Library>,
		context: std::sync::Arc<CoreContext>,
	) -> Result<(), ActionError> {
		if !self.input.path.exists() {
			return Err(ActionError::Validation {
				field: "path".to_string(),
				message: "Path does not exist".to_string(),
			});
		}
		if !self.input.path.is_dir() {
			return Err(ActionError::Validation {
				field: "path".to_string(),
				message: "Path must be a directory".to_string(),
			});
		}
		Ok(())
	}
}

// Register action
crate::register_library_action!(LocationAddAction, "locations.add");
