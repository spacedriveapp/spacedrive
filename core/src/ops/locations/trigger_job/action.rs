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
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
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
	Ocr,
	SpeechToText,
	ObjectDetection,
}

impl std::fmt::Display for JobType {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			JobType::Thumbnail => write!(f, "thumbnail"),
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
			JobType::Thumbnail => {
				if !job_policies.thumbnail.enabled && !self.input.force {
					return Err(ActionError::Validation {
						field: "job_type".to_string(),
						message: "Thumbnail generation is disabled for this location. Use force=true to override.".to_string(),
					});
				}

				let config = job_policies.thumbnail.to_job_config();
				let job = crate::ops::media::thumbnail::ThumbnailJob::new(config);

				library
					.jobs()
					.dispatch(job)
					.await
					.map_err(|e| ActionError::Internal(format!("Failed to dispatch thumbnail job: {}", e)))?
			}

			JobType::Ocr => {
				if !job_policies.ocr.enabled && !self.input.force {
					return Err(ActionError::Validation {
						field: "job_type".to_string(),
						message: "OCR is disabled for this location. Use force=true to override.".to_string(),
					});
				}

				let config = job_policies.ocr.to_job_config(Some(self.input.location_id));
				let job = crate::ops::media::ocr::OcrJob::new(config);

				library
					.jobs()
					.dispatch(job)
					.await
					.map_err(|e| ActionError::Internal(format!("Failed to dispatch OCR job: {}", e)))?
			}

			JobType::SpeechToText => {
				if !job_policies.speech_to_text.enabled && !self.input.force {
					return Err(ActionError::Validation {
						field: "job_type".to_string(),
						message: "Speech-to-text is disabled for this location. Use force=true to override.".to_string(),
					});
				}

				let config = job_policies.speech_to_text.to_job_config(Some(self.input.location_id));
				let job = crate::ops::media::speech::SpeechToTextJob::new(config);

				library
					.jobs()
					.dispatch(job)
					.await
					.map_err(|e| ActionError::Internal(format!("Failed to dispatch speech-to-text job: {}", e)))?
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
		library: std::sync::Arc<crate::library::Library>,
		context: std::sync::Arc<crate::context::CoreContext>,
	) -> Result<(), ActionError> {
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

		Ok(())
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

// Register action
crate::register_library_action!(LocationTriggerJobAction, "locations.triggerJob");
