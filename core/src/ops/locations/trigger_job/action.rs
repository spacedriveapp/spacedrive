//! Location trigger job action handler

use super::output::LocationTriggerJobOutput;
use crate::{
	context::CoreContext,
	infra::action::{
		context::ActionContextProvider,
		error::{ActionError, ActionResult},
		LibraryAction,
	},
	infra::db::entities,
};
use async_trait::async_trait;
use sea_orm::{
	ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter, Statement,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use specta::Type;
use std::sync::Arc;
use uuid::Uuid;

/// Type of job to trigger for a location
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JobType {
	Thumbnail,
	Thumbstrip,
	Ocr,
	SpeechToText,
	ObjectDetection,
}

impl std::fmt::Display for JobType {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			JobType::Thumbnail => write!(f, "thumbnail"),
			JobType::Thumbstrip => write!(f, "thumbstrip"),
			JobType::Ocr => write!(f, "ocr"),
			JobType::SpeechToText => write!(f, "speech_to_text"),
			JobType::ObjectDetection => write!(f, "object_detection"),
		}
	}
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LocationTriggerJobInput {
	/// UUID of the location to run the job on
	pub location_id: Uuid,

	/// Type of job to trigger
	pub job_type: JobType,

	/// Force the job to run even if disabled in the location's policy
	#[serde(default)]
	pub force: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationTriggerJobAction {
	input: LocationTriggerJobInput,
}

impl LocationTriggerJobAction {
	pub fn new(input: LocationTriggerJobInput) -> Self {
		Self { input }
	}
}

impl LibraryAction for LocationTriggerJobAction {
	type Input = LocationTriggerJobInput;
	type Output = LocationTriggerJobOutput;

	fn from_input(input: LocationTriggerJobInput) -> Result<Self, String> {
		Ok(LocationTriggerJobAction::new(input))
	}

	async fn execute(
		self,
		library: std::sync::Arc<crate::library::Library>,
		context: std::sync::Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		use crate::domain::location::JobPolicies;

		let db = library.db().conn();

		// Find the location by UUID
		let location = entities::location::Entity::find()
			.filter(entities::location::Column::Uuid.eq(self.input.location_id))
			.one(db)
			.await
			.map_err(ActionError::SeaOrm)?
			.ok_or_else(|| ActionError::LocationNotFound(self.input.location_id))?;

		// Parse job policies
		let job_policies: JobPolicies = location
			.job_policies
			.as_ref()
			.and_then(|json| serde_json::from_str(json).ok())
			.unwrap_or_default();

		// Dispatch the appropriate job based on type
		let job_handle = match self.input.job_type {
			#[cfg(feature = "ffmpeg")]
			JobType::Thumbnail => {
				if !job_policies.thumbnail.enabled && !self.input.force {
					return Err(ActionError::Validation {
						field: "job_type".to_string(),
						message: "Thumbnail generation is disabled for this location. Use force=true to override.".to_string(),
					});
				}

				// Query entries for this location to avoid processing all database entries
				let entry_uuids = query_location_entry_uuids(db, self.input.location_id).await?;

				let config = job_policies.thumbnail.to_job_config();
				let job = if entry_uuids.is_empty() {
					// No entries in location, but still dispatch job to log this
					crate::ops::media::thumbnail::ThumbnailJob::new(config)
				} else {
					crate::ops::media::thumbnail::ThumbnailJob::for_entries(entry_uuids, config)
				};

				library.jobs().dispatch(job).await.map_err(|e| {
					ActionError::Internal(format!("Failed to dispatch thumbnail job: {}", e))
				})?
			}

			#[cfg(feature = "ffmpeg")]
			JobType::Thumbstrip => {
				if !job_policies.thumbstrip.enabled && !self.input.force {
					return Err(ActionError::Validation {
						field: "job_type".to_string(),
						message: "Thumbstrip generation is disabled for this location. Use force=true to override.".to_string(),
					});
				}

				// Query entries for this location to avoid processing all database entries
				let entry_uuids = query_location_entry_uuids(db, self.input.location_id).await?;

				let config = job_policies.thumbstrip.to_job_config();
				let job = if entry_uuids.is_empty() {
					// No entries in location, but still dispatch job to log this
					crate::ops::media::thumbstrip::ThumbstripJob::new(config)
				} else {
					crate::ops::media::thumbstrip::ThumbstripJob::for_entries(entry_uuids, config)
				};

				library.jobs().dispatch(job).await.map_err(|e| {
					ActionError::Internal(format!("Failed to dispatch thumbstrip job: {}", e))
				})?
			}

			JobType::Ocr => {
				if !job_policies.ocr.enabled && !self.input.force {
					return Err(ActionError::Validation {
						field: "job_type".to_string(),
						message: "OCR is disabled for this location. Use force=true to override."
							.to_string(),
					});
				}

				let config = job_policies.ocr.to_job_config(Some(self.input.location_id));
				let job = crate::ops::media::ocr::OcrJob::new(config);

				library.jobs().dispatch(job).await.map_err(|e| {
					ActionError::Internal(format!("Failed to dispatch OCR job: {}", e))
				})?
			}

			#[cfg(feature = "speech-to-text")]
			JobType::SpeechToText => {
				if !job_policies.speech_to_text.enabled && !self.input.force {
					return Err(ActionError::Validation {
						field: "job_type".to_string(),
						message: "Speech-to-text is disabled for this location. Use force=true to override.".to_string(),
					});
				}

				let config = job_policies
					.speech_to_text
					.to_job_config(Some(self.input.location_id));
				let job = crate::ops::media::speech::SpeechToTextJob::new(config);

				library.jobs().dispatch(job).await.map_err(|e| {
					ActionError::Internal(format!("Failed to dispatch speech-to-text job: {}", e))
				})?
			}

			#[cfg(not(feature = "ffmpeg"))]
			JobType::Thumbnail | JobType::Thumbstrip => {
				return Err(ActionError::Validation {
					field: "job_type".to_string(),
					message: format!(
						"{} requires FFmpeg support which is not enabled",
						self.input.job_type
					),
				});
			}

			#[cfg(not(feature = "speech-to-text"))]
			JobType::SpeechToText => {
				return Err(ActionError::Validation {
					field: "job_type".to_string(),
					message:
						"Speech-to-text requires FFmpeg and Whisper support which is not enabled"
							.to_string(),
				});
			}

			JobType::ObjectDetection => {
				return Err(ActionError::Validation {
					field: "job_type".to_string(),
					message: "Object detection is not yet implemented".to_string(),
				});
			}
		};

		Ok(LocationTriggerJobOutput {
			job_id: job_handle.id().into(),
			job_type: self.input.job_type,
			location_id: self.input.location_id,
		})
	}

	fn action_kind(&self) -> &'static str {
		"locations.triggerJob"
	}

	async fn validate(
		&self,
		library: &std::sync::Arc<crate::library::Library>,
		context: std::sync::Arc<crate::context::CoreContext>,
	) -> Result<crate::infra::action::ValidationResult, ActionError> {
		// Validate that the location exists
		let db = library.db().conn();
		let exists = entities::location::Entity::find()
			.filter(entities::location::Column::Uuid.eq(self.input.location_id))
			.one(db)
			.await
			.map_err(ActionError::SeaOrm)?
			.is_some();

		if !exists {
			return Err(ActionError::LocationNotFound(self.input.location_id));
		}

		Ok(crate::infra::action::ValidationResult::Success { metadata: None })
	}
}

impl ActionContextProvider for LocationTriggerJobAction {
	fn create_action_context(&self) -> crate::infra::action::context::ActionContext {
		use crate::infra::action::context::{sanitize_action_input, ActionContext};

		ActionContext::new(
			Self::action_type_name(),
			sanitize_action_input(&self.input),
			json!({
				"operation": "trigger_job",
				"trigger": "user_action",
				"location_id": self.input.location_id,
				"job_type": self.input.job_type.to_string(),
				"force": self.input.force,
			}),
		)
	}

	fn action_type_name() -> &'static str
	where
		Self: Sized,
	{
		"locations.triggerJob"
	}
}

/// Helper function to query entry UUIDs for a specific location
async fn query_location_entry_uuids(
	db: &DatabaseConnection,
	location_id: Uuid,
) -> Result<Vec<Uuid>, ActionError> {
	use crate::infra::db::entities::{entry, location};

	// Find the location's entry_id (root entry)
	let location_record = location::Entity::find()
		.filter(location::Column::Uuid.eq(location_id))
		.one(db)
		.await
		.map_err(ActionError::SeaOrm)?
		.ok_or_else(|| ActionError::LocationNotFound(location_id))?;

	let root_entry_id = location_record
		.entry_id
		.ok_or_else(|| ActionError::Internal("Location has no root entry".to_string()))?;

	// Query all entry IDs that are descendants of this location's root entry
	// using the entry_closure table
	let entry_ids: Vec<i32> = db
		.query_all(Statement::from_sql_and_values(
			sea_orm::DbBackend::Sqlite,
			"SELECT descendant_id FROM entry_closure WHERE ancestor_id = ?",
			vec![root_entry_id.into()],
		))
		.await
		.map_err(ActionError::SeaOrm)?
		.iter()
		.filter_map(|row| row.try_get_by_index::<i32>(0).ok())
		.collect();

	if entry_ids.is_empty() {
		return Ok(Vec::new());
	}

	// Now get the UUIDs for these entry IDs
	let entry_models = entry::Entity::find()
		.filter(entry::Column::Id.is_in(entry_ids))
		.all(db)
		.await
		.map_err(ActionError::SeaOrm)?;

	let uuids: Vec<Uuid> = entry_models.into_iter().filter_map(|e| e.uuid).collect();

	Ok(uuids)
}

// Register action
crate::register_library_action!(LocationTriggerJobAction, "locations.triggerJob");
