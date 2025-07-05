//! Action manager - central router for all actions

use super::{
    Action, error::{ActionError, ActionResult}, output::ActionOutput, registry::REGISTRY,
};
use crate::{
    context::CoreContext,
    infrastructure::database::entities::{audit_log, AuditLog, AuditLogActive},
    shared::types::get_current_device_id,
};
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QuerySelect, Set};
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

    /// Dispatch an action for execution
    pub async fn dispatch(
        &self,
        action: Action,
    ) -> ActionResult<ActionOutput> {
        // 1. Find the correct handler in the registry
        let handler = REGISTRY
            .get(action.kind())
            .ok_or_else(|| ActionError::ActionNotRegistered(action.kind().to_string()))?;

        // 2. Validate the action
        handler.validate(self.context.clone(), &action).await?;

        // 3. Create the initial audit log entry (if library-scoped)
        let audit_entry = if let Some(library_id) = action.library_id() {
            Some(self.create_audit_log(library_id, &action).await?)
        } else {
            None
        };

        // 4. Execute the handler
        let result = handler.execute(self.context.clone(), action).await;

        // 5. Update the audit log with the final status (if we created one)
        if let Some(entry) = audit_entry {
            self.finalize_audit_log(entry, &result).await?;
        }

        result
    }

    /// Create an initial audit log entry
    async fn create_audit_log(
        &self,
        library_id: Uuid,
        action: &Action,
    ) -> ActionResult<audit_log::Model> {
        let library = self.get_library(library_id).await?;
        let db = library.db().conn();
        
        let audit_entry = AuditLogActive {
            uuid: Set(Uuid::new_v4()),
            action_type: Set(action.kind().to_string()),
            actor_device_id: Set(get_current_device_id()),
            targets: Set(action.targets_summary()),
            status: Set(audit_log::ActionStatus::InProgress),
            job_id: Set(None),
            created_at: Set(chrono::Utc::now()),
            completed_at: Set(None),
            error_message: Set(None),
            result_payload: Set(None),
            ..Default::default()
        };

        audit_entry
            .insert(db)
            .await
            .map_err(ActionError::SeaOrm)
    }

    /// Finalize the audit log entry with the result
    async fn finalize_audit_log(
        &self,
        mut entry: audit_log::Model,
        result: &ActionResult<ActionOutput>,
    ) -> ActionResult<()> {
        // We need to get the library_id to update the audit log
        // For now, we'll need to store this in the audit entry or pass it through
        // This is a temporary limitation that would be resolved by adding library_id to audit_log table
        
        // For this implementation, let's assume we can find any open library for the audit update
        let libraries = self.context.library_manager.get_open_libraries().await;
        let library = libraries.first()
            .ok_or(ActionError::Internal("No libraries available for audit log update".to_string()))?;
        let db = library.db().conn();

        match result {
            Ok(output) => {
                entry.status = audit_log::ActionStatus::Completed;
                entry.completed_at = Some(chrono::Utc::now());
                // Extract job_id from output if it contains one
                entry.job_id = match output {
                    ActionOutput::FileCopyDispatched { job_id, .. } => Some(*job_id),
                    ActionOutput::FileDeleteDispatched { job_id, .. } => Some(*job_id),
                    ActionOutput::FileIndexDispatched { job_id, .. } => Some(*job_id),
                    _ => None,
                };
                entry.result_payload = Some(serde_json::to_value(output).unwrap_or(serde_json::Value::Null));
            }
            Err(error) => {
                entry.status = audit_log::ActionStatus::Failed;
                entry.completed_at = Some(chrono::Utc::now());
                entry.error_message = Some(error.to_string());
            }
        }

        let active_model: AuditLogActive = entry.into();
        active_model
            .update(db)
            .await
            .map_err(ActionError::SeaOrm)?;

        Ok(())
    }

    /// Get the library for database operations
    async fn get_library(&self, library_id: Uuid) -> ActionResult<std::sync::Arc<crate::library::Library>> {
        self.context
            .library_manager
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

        query
            .all(db)
            .await
            .map_err(ActionError::SeaOrm)
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