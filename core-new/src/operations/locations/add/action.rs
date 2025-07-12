//! Location add action handler

use super::output::LocationAddOutput;
use crate::{
	context::CoreContext,
	infrastructure::actions::{
		error::{ActionError, ActionResult},
		handler::ActionHandler,
		output::ActionOutput,
		Action,
	},
	infrastructure::database::entities,
	location::manager::LocationManager,
	operations::indexing::IndexMode,
	register_action_handler,
};
use async_trait::async_trait;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationAddAction {
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

#[async_trait]
impl ActionHandler for LocationAddHandler {
	async fn validate(&self, _context: Arc<CoreContext>, action: &Action) -> ActionResult<()> {
		if let Action::LocationAdd {
			library_id: _,
			action,
		} = action
		{
			if !action.path.exists() {
				return Err(ActionError::Validation {
					field: "path".to_string(),
					message: "Path does not exist".to_string(),
				});
			}
			if !action.path.is_dir() {
				return Err(ActionError::Validation {
					field: "path".to_string(),
					message: "Path must be a directory".to_string(),
				});
			}
			Ok(())
		} else {
			Err(ActionError::InvalidActionType)
		}
	}

	async fn execute(
		&self,
		context: Arc<CoreContext>,
		action: Action,
	) -> ActionResult<ActionOutput> {
		if let Action::LocationAdd { library_id, action } = action {
			let library_manager = &context.library_manager;

			// Get the specific library
			let library = library_manager
				.get_library(library_id)
				.await
				.ok_or(ActionError::LibraryNotFound(library_id))?;

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

			let location_mode = match action.mode {
				IndexMode::Shallow => crate::location::IndexMode::Shallow,
				IndexMode::Content => crate::location::IndexMode::Content,
				IndexMode::Deep => crate::location::IndexMode::Deep,
			};

			// Store the name to use for output since we're moving it
			let name_for_output = action.name.clone();

			let (location_id, job_id_string) = location_manager
				.add_location(
					library.clone(),
					action.path.clone(),
					action.name,
					device_record.id,
					location_mode,
				)
				.await
				.map_err(|e| ActionError::Internal(e.to_string()))?;

			// Parse the job ID from the string returned by add_location
			let job_id = if !job_id_string.is_empty() {
				Uuid::parse_str(&job_id_string)
					.map_err(|e| ActionError::Internal(format!("Failed to parse job ID: {}", e)))?
			} else {
				// If no job was created by add_location, we should handle this case
				return Err(ActionError::Internal("Location added but indexing failed to start".to_string()));
			};

			let output = LocationAddOutput::new(location_id, action.path.clone(), name_for_output)
				.with_job_id(job_id.into());
			Ok(ActionOutput::from_trait(output))
		} else {
			Err(ActionError::InvalidActionType)
		}
	}

	fn can_handle(&self, action: &Action) -> bool {
		matches!(action, Action::LocationAdd { .. })
	}

	fn supported_actions() -> &'static [&'static str] {
		&["location.add"]
	}
}

// Register this handler
register_action_handler!(LocationAddHandler, "location.add");
