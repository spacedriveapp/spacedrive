//! Location rescan action handler

use crate::{
	context::CoreContext,
	domain::addressing::SdPath,
	infra::{
		action::{error::ActionError, LibraryAction},
		db::entities,
		job::handle::JobHandle,
	},
	ops::indexing::{job::IndexerJob, IndexMode, PathResolver},
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LocationRescanInput {
	pub location_id: Uuid,
	pub full_rescan: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationRescanAction {
	input: LocationRescanInput,
}

impl LocationRescanAction {
	pub fn new(input: LocationRescanInput) -> Self {
		Self { input }
	}
}

// Implement LibraryAction
impl LibraryAction for LocationRescanAction {
	type Input = LocationRescanInput;
	type Output = super::output::LocationRescanOutput;

	fn from_input(input: LocationRescanInput) -> Result<Self, String> {
		Ok(LocationRescanAction::new(input))
	}

	async fn execute(
		self,
		library: std::sync::Arc<crate::library::Library>,
		_context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

		// Get location details from database
		let location = entities::location::Entity::find()
			.filter(entities::location::Column::Uuid.eq(self.input.location_id))
			.one(library.db().conn())
			.await
			.map_err(|e| ActionError::Internal(format!("Database error: {}", e)))?
			.ok_or_else(|| {
				ActionError::Internal(format!("Location not found: {}", self.input.location_id))
			})?;

		// Get the location's path using PathResolver
		let entry_id = location.entry_id.ok_or_else(|| {
			ActionError::Internal("Location entry_id not set (not yet synced)".to_string())
		})?;
		let location_path_buf = PathResolver::get_full_path(library.db().conn(), entry_id)
			.await
			.map_err(|e| ActionError::Internal(format!("Failed to get location path: {}", e)))?;
		let location_path_str = location_path_buf.to_string_lossy().to_string();
		let location_path = SdPath::from_uri(&location_path_str)
			.map_err(|e| ActionError::Internal(format!("Failed to parse location path: {}", e)))?;

		// Determine index mode based on full_rescan flag
		let mode = if self.input.full_rescan {
			IndexMode::Deep
		} else {
			match location.index_mode.as_str() {
				"shallow" => IndexMode::Shallow,
				"content" => IndexMode::Content,
				"deep" => IndexMode::Deep,
				_ => IndexMode::Deep,
			}
		};

		// Create indexer job for rescan
		let job = IndexerJob::from_location(self.input.location_id, location_path, mode);

		// Dispatch the job
		let job_handle = library
			.jobs()
			.dispatch(job)
			.await
			.map_err(ActionError::Job)?;

		Ok(super::output::LocationRescanOutput {
			location_id: self.input.location_id,
			location_path: location_path_str,
			job_id: job_handle.id().into(),
			full_rescan: self.input.full_rescan,
		})
	}

	fn action_kind(&self) -> &'static str {
		"location.rescan"
	}
}

// Register action
crate::register_library_action!(LocationRescanAction, "locations.rescan");
