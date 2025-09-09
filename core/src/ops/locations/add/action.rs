//! Location add action handler

use super::output::LocationAddOutput;
use crate::{
	context::CoreContext,
	infra::action::{
		error::{ActionError, ActionResult},
		ActionTrait,
	},
	infra::db::entities,
	location::manager::LocationManager,
	ops::indexing::IndexMode,
	register_action_handler,
};
use async_trait::async_trait;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationAddAction {
	pub library_id: Uuid,
	pub path: PathBuf,
	pub name: Option<String>,
	pub mode: IndexMode,
}

pub struct LocationAddHandler;

impl LocationAddHandler {
	pub fn new() -> Self {
		Self
	}
}

// Note: ActionHandler implementation removed - using ActionType instead

// Implement the new modular ActionType trait
impl ActionTrait for LocationAddAction {
	type Output = LocationAddOutput;

	async fn execute(self, context: std::sync::Arc<CoreContext>) -> Result<Self::Output, ActionError> {
		// Get the specific library
		let library = context
			.library_manager
			.get_library(self.library_id)
			.await
			.ok_or(ActionError::LibraryNotFound(self.library_id))?;

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

		let location_mode = match self.mode {
			IndexMode::Shallow => crate::location::IndexMode::Shallow,
			IndexMode::Content => crate::location::IndexMode::Content,
			IndexMode::Deep => crate::location::IndexMode::Deep,
		};

		let (location_id, job_id_string) = location_manager
			.add_location(
				library.clone(),
				self.path.clone(),
				self.name.clone(),
				device_record.id,
				location_mode,
			)
			.await
			.map_err(|e| ActionError::Internal(e.to_string()))?;

		// Parse the job ID from the string returned by add_location
		let job_id = if !job_id_string.is_empty() {
			Some(Uuid::parse_str(&job_id_string)
				.map_err(|e| ActionError::Internal(format!("Failed to parse job ID: {}", e)))?)
		} else {
			None
		};

		// Return native output directly
		Ok(LocationAddOutput::new(
			location_id,
			self.path,
			self.name,
		))
	}

	fn action_kind(&self) -> &'static str {
		"location.add"
	}

	fn library_id(&self) -> Option<Uuid> {
		Some(self.library_id)
	}

	async fn validate(&self, _context: std::sync::Arc<CoreContext>) -> Result<(), ActionError> {
		if !self.path.exists() {
			return Err(ActionError::Validation {
				field: "path".to_string(),
				message: "Path does not exist".to_string(),
			});
		}
		if !self.path.is_dir() {
			return Err(ActionError::Validation {
				field: "path".to_string(),
				message: "Path must be a directory".to_string(),
			});
		}
		Ok(())
	}
}
