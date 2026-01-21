//! Action manager - central router for all actions

use super::error::{ActionError, ActionResult};
use crate::{
	context::CoreContext,
	infra::db::entities::{audit_log, AuditLog, AuditLogActive},
};
use sea_orm::{
	ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QuerySelect, Set,
};
use std::sync::Arc;
use uuid::Uuid;

/// Central manager for all action execution
pub struct ActionManager {
	context: Arc<CoreContext>,
}

impl ActionManager {
	/// Create a new action manager
	pub fn new(context: Arc<CoreContext>) -> Self {
		Self { context }
	}

	/// Dispatch a core-level action (no library context required)
	pub async fn dispatch_core<A: super::CoreAction>(
		&self,
		action: A,
	) -> Result<A::Output, super::error::ActionError> {
		// Log action execution (capture action_kind before move)
		let action_kind = action.action_kind();
		tracing::info!("Executing core action: {}", action_kind);

		// Validate the action first
		let validation_result = action.validate(self.context.clone()).await?;

		// Check if confirmation is required
		match validation_result {
			super::ValidationResult::Success { .. } => {
				// Proceed with execution
			}
			super::ValidationResult::RequiresConfirmation(_request) => {
				// Cannot handle confirmation in this context
				// The dispatcher/CLI layer should have called validate_core first
				return Err(ActionError::Internal(
					"Action requires confirmation but confirmation was not resolved".to_string(),
				));
			}
		}

		// Execute the action directly
		let result = action.execute(self.context.clone()).await;

		// Log result
		match &result {
			Ok(_) => tracing::info!("Core action {} completed successfully", action_kind),
			Err(e) => tracing::error!("Core action {} failed: {}", action_kind, e),
		}

		result
	}

	/// Validate a core action and return the validation result
	/// This allows checking for confirmations before executing
	pub async fn validate_core<A: super::CoreAction>(
		&self,
		action: &A,
	) -> Result<super::ValidationResult, super::error::ActionError> {
		action.validate(self.context.clone()).await
	}

	/// Dispatch a library-scoped action (library context pre-validated)
	pub async fn dispatch_library<A: super::LibraryAction>(
		&self,
		library_id: Option<Uuid>,
		action: A,
	) -> Result<A::Output, super::error::ActionError> {
		let library_id =
			library_id.ok_or(ActionError::LibraryNotFound(library_id.unwrap_or_default()))?;
		// Get and validate library exists
		let library = self
			.context
			.get_library(library_id)
			.await
			.ok_or_else(|| ActionError::LibraryNotFound(library_id))?;

		// Create audit log entry (capture values before move)
		let action_kind = action.action_kind();
		let audit_entry = self
			.create_action_audit_log(library_id, action_kind)
			.await?;

		// Validate the action first
		let validation_result = action.validate(&library, self.context.clone()).await?;

		// Check if confirmation is required
		match validation_result {
			super::ValidationResult::Success { .. } => {
				// Proceed with execution
			}
			super::ValidationResult::RequiresConfirmation(_request) => {
				// Cannot handle confirmation in this context
				// The dispatcher/CLI layer should have called validate_library first
				return Err(ActionError::Internal(
					"Action requires confirmation but confirmation was not resolved".to_string(),
				));
			}
		}

		// Execute the action with validated library
		let result = action.execute(library, self.context.clone()).await;

		// Finalize audit log with result
		let audit_result = match &result {
			Ok(_) => Ok("Action completed successfully".to_string()),
			Err(e) => Err(ActionError::Internal(e.to_string())),
		};
		self.finalize_audit_log(audit_entry, &audit_result, library_id)
			.await?;

		result
	}

	/// Validate a library action and return the validation result
	/// This allows checking for confirmations before executing
	pub async fn validate_library<A: super::LibraryAction>(
		&self,
		library_id: Option<Uuid>,
		action: &A,
	) -> Result<super::ValidationResult, super::error::ActionError> {
		let library_id =
			library_id.ok_or(ActionError::LibraryNotFound(library_id.unwrap_or_default()))?;
		// Get and validate library exists
		let library = self
			.context
			.get_library(library_id)
			.await
			.ok_or_else(|| ActionError::LibraryNotFound(library_id))?;

		// Validate the action
		action.validate(&library, self.context.clone()).await
	}

	/// Create an initial audit log entry for ActionTrait
	async fn create_action_audit_log(
		&self,
		library_id: Uuid,
		action_kind: &str,
	) -> ActionResult<audit_log::Model> {
		let library = self.get_library(library_id).await?;
		let db = library.db().conn();

		let device_id = crate::device::get_current_device_id();
		let audit_entry = AuditLogActive {
			uuid: Set(Uuid::new_v4().to_string()),
			action_type: Set(action_kind.to_string()),
			actor_device_id: Set(device_id.to_string()),
			targets: Set("{}".to_string()), // TODO: Add targets_summary to ActionTrait
			status: Set(audit_log::ActionStatus::InProgress),
			job_id: Set(None),
			created_at: Set(chrono::Utc::now()),
			completed_at: Set(None),
			error_message: Set(None),
			result_payload: Set(None),
			version: Set(1),
			..Default::default()
		};

		let model = audit_entry.insert(db).await.map_err(ActionError::SeaOrm)?;

		// Sync the audit log creation
		library
			.sync_model(&model, crate::infra::sync::ChangeType::Insert)
			.await
			.map_err(|e| ActionError::Internal(format!("Failed to sync audit log: {}", e)))?;

		Ok(model)
	}

	/// Finalize the audit log entry with the result
	async fn finalize_audit_log(
		&self,
		mut entry: audit_log::Model,
		result: &ActionResult<String>,
		library_id: Uuid,
	) -> ActionResult<()> {
		let library = self.get_library(library_id).await?;
		let db = library.db().conn();

		match result {
			Ok(_) => {
				entry.status = audit_log::ActionStatus::Completed;
				entry.completed_at = Some(chrono::Utc::now());
			}
			Err(error) => {
				entry.status = audit_log::ActionStatus::Failed;
				entry.completed_at = Some(chrono::Utc::now());
				entry.error_message = Some(error.to_string());
			}
		}

		// Convert to active model and explicitly mark changed fields
		let mut active_model: AuditLogActive = entry.into();

		// Explicitly mark the fields we want to update as "Set" (changed)
		match result {
			Ok(output) => {
				active_model.status = Set(audit_log::ActionStatus::Completed);
				active_model.completed_at = Set(Some(chrono::Utc::now()));
				active_model.result_payload = Set(Some(output.clone()));
				// Increment version for sync conflict resolution
				active_model.version = Set(active_model.version.clone().unwrap() + 1);
			}
			Err(error) => {
				active_model.status = Set(audit_log::ActionStatus::Failed);
				active_model.completed_at = Set(Some(chrono::Utc::now()));
				active_model.error_message = Set(Some(error.to_string()));
				// Increment version for sync conflict resolution
				active_model.version = Set(active_model.version.clone().unwrap() + 1);
			}
		}

		let updated = active_model.update(db).await.map_err(ActionError::SeaOrm)?;

		// Sync the update
		library
			.sync_model(&updated, crate::infra::sync::ChangeType::Update)
			.await
			.map_err(|e| {
				ActionError::Internal(format!("Failed to sync audit log update: {}", e))
			})?;

		Ok(())
	}

	/// Get the library for database operations
	async fn get_library(
		&self,
		library_id: Uuid,
	) -> ActionResult<std::sync::Arc<crate::library::Library>> {
		self.context
			.get_library(library_id)
			.await
			.ok_or(ActionError::LibraryNotFound(library_id))
	}

	/// Get action history for a library
	pub async fn get_action_history(
		&self,
		library_id: Uuid,
		limit: Option<u64>,
		offset: Option<u64>,
	) -> ActionResult<Vec<audit_log::Model>> {
		let library = self.get_library(library_id).await?;
		let db = library.db().conn();

		let mut query = AuditLog::find();

		if let Some(limit) = limit {
			query = query.limit(limit);
		}

		if let Some(offset) = offset {
			query = query.offset(offset);
		}

		query.all(db).await.map_err(ActionError::SeaOrm)
	}

	/// Get specific action by UUID
	pub async fn get_action(
		&self,
		library_id: Uuid,
		action_uuid: Uuid,
	) -> ActionResult<Option<audit_log::Model>> {
		let library = self.get_library(library_id).await?;
		let db = library.db().conn();

		AuditLog::find()
			.filter(audit_log::Column::Uuid.eq(action_uuid))
			.one(db)
			.await
			.map_err(ActionError::SeaOrm)
	}
}
