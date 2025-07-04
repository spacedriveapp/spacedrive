//! Location add action handler

use crate::{
    context::CoreContext,
    infrastructure::database::entities,
    location::manager::LocationManager,
    operations::{
        actions::{
            Action, error::{ActionError, ActionResult}, handler::ActionHandler, receipt::ActionReceipt,
        },
        indexing::{IndexMode as CoreIndexMode, IndexScope, job::{IndexerJob, IndexerJobConfig}},
    },
    register_action_handler,
    shared::types::SdPath,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use async_trait::async_trait;
use std::sync::Arc;
use uuid::Uuid;

pub struct LocationAddHandler;

impl LocationAddHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ActionHandler for LocationAddHandler {
    async fn validate(
        &self,
        _context: Arc<CoreContext>,
        action: &Action,
    ) -> ActionResult<()> {
        if let Action::LocationAdd { path, .. } = action {
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
            Ok(())
        } else {
            Err(ActionError::InvalidActionType)
        }
    }

    async fn execute(
        &self,
        context: Arc<CoreContext>,
        action: Action,
    ) -> ActionResult<ActionReceipt> {
        if let Action::LocationAdd { library_id, path, name, mode } = action {
            let library_manager = &context.library_manager;
            
            // Get the specific library
            let library = library_manager
                .get_library(library_id)
                .await
                .ok_or(ActionError::LibraryNotFound(library_id))?;

            // Get the device UUID from the device manager
            let device_uuid = context.device_manager
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
            
            let location_mode = match mode {
                crate::operations::actions::IndexMode::Shallow => crate::location::IndexMode::Shallow,
                crate::operations::actions::IndexMode::Deep => crate::location::IndexMode::Deep,
                crate::operations::actions::IndexMode::Sync => crate::location::IndexMode::Deep, // Default fallback
            };
            
            let (location_id, location_name) = location_manager
                .add_location(library.clone(), path.clone(), name, device_record.id, location_mode)
                .await
                .map_err(|e| ActionError::Internal(e.to_string()))?;

            // Now dispatch an indexing job based on the mode
            let job_handle = if matches!(mode, crate::operations::actions::IndexMode::Sync) {
                // Sync mode doesn't automatically start indexing
                None
            } else {
                // Convert action mode to core indexing mode
                let core_mode = match mode {
                    crate::operations::actions::IndexMode::Shallow => CoreIndexMode::Shallow,
                    crate::operations::actions::IndexMode::Deep => CoreIndexMode::Deep,
                    crate::operations::actions::IndexMode::Sync => CoreIndexMode::Deep, // Fallback, but won't be used
                };

                // Create indexer job configuration
                let indexer_config = IndexerJobConfig {
                    location_id: Some(location_id),
                    path: SdPath::local(path.clone()),
                    mode: core_mode,
                    scope: IndexScope::Recursive, // Default to recursive for location indexing
                    persistence: crate::operations::indexing::IndexPersistence::Persistent,
                    max_depth: None,
                };

                // Dispatch the indexer job
                let job_params = serde_json::to_value(&indexer_config)
                    .map_err(ActionError::JsonSerialization)?;

                let job_handle = library
                    .jobs()
                    .dispatch_by_name("indexer", job_params)
                    .await
                    .map_err(ActionError::Job)?;

                Some(job_handle)
            };

            Ok(ActionReceipt::hybrid(
                Uuid::new_v4(),
                Some(serde_json::json!({
                    "location_id": location_id,
                    "name": location_name,
                    "path": path.display().to_string()
                })),
                job_handle,
            ))
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