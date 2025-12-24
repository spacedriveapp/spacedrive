//! Trigger stale detection action

use crate::{
	context::CoreContext,
	domain::addressing::SdPath,
	domain::location::{IndexMode, StaleDetectionTrigger},
	infra::{
		action::{error::ActionError, LibraryAction, ValidationResult},
		job::handle::JobReceipt,
	},
	library::Library,
	ops::indexing::{
		job::{IndexPersistence, IndexScope, IndexerJob, IndexerJobConfig},
		PathResolver,
	},
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;
use tracing::info;
use uuid::Uuid;

/// Input for triggering stale detection
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct TriggerStaleDetectionInput {
	pub location_id: Uuid,
}

/// Output containing job receipt
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct TriggerStaleDetectionOutput {
	pub job_id: String,
}

/// Action to manually trigger stale detection for a location
#[derive(Debug, Clone)]
pub struct TriggerStaleDetectionAction {
	pub input: TriggerStaleDetectionInput,
}

impl LibraryAction for TriggerStaleDetectionAction {
	type Input = TriggerStaleDetectionInput;
	type Output = TriggerStaleDetectionOutput;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		Ok(Self { input })
	}

	async fn validate(
		&self,
		_library: &Arc<Library>,
		_context: Arc<CoreContext>,
	) -> Result<ValidationResult, ActionError> {
		Ok(ValidationResult::Success)
	}

	async fn execute(
		self,
		library: Arc<Library>,
		_context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let db = library.db().conn();

		// Get location
		use crate::infra::db::entities::location;
		let loc = location::Entity::find()
			.filter(location::Column::Uuid.eq(self.input.location_id))
			.one(db)
			.await?
			.ok_or_else(|| ActionError::LocationNotFound(self.input.location_id))?;

		// Get entry_id for path resolution
		let entry_id = loc.entry_id.ok_or_else(|| {
			ActionError::InvalidInput("Location has no root entry".to_string())
		})?;

		// Get the filesystem path
		let path = PathResolver::get_full_path(db, entry_id).await.map_err(|e| {
			ActionError::Internal(format!("Failed to resolve location path: {}", e))
		})?;

		// Get location's configured index mode
		let location_index_mode = IndexMode::from_str(&loc.index_mode);

		info!(
			location_id = %self.input.location_id,
			path = %path.display(),
			index_mode = ?location_index_mode,
			"Triggering manual stale detection"
		);

		// Create IndexerJob config with Stale mode wrapping the location's mode
		let config = IndexerJobConfig {
			location_id: Some(self.input.location_id),
			path: SdPath::local(path),
			mode: IndexMode::Stale(Box::new(location_index_mode)),
			scope: IndexScope::Recursive,
			persistence: IndexPersistence::Persistent,
			max_depth: None,
			rule_toggles: Default::default(),
		};

		let job = IndexerJob::new(config);
		let handle = library
			.jobs()
			.dispatch(job)
			.await
			.map_err(ActionError::Job)?;

		let job_id = handle.id().to_string();

		info!(
			location_id = %self.input.location_id,
			job_id = %job_id,
			"Stale detection job dispatched"
		);

		Ok(TriggerStaleDetectionOutput { job_id })
	}

	fn action_kind(&self) -> &'static str {
		"locations.triggerStaleDetection"
	}
}

crate::register_library_action!(TriggerStaleDetectionAction, "locations.triggerStaleDetection");
