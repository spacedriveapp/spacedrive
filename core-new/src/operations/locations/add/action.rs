//! Location add action handler

use crate::{
    context::CoreContext,
    infrastructure::database::entities,
    location::manager::LocationManager,
    infrastructure::{
        actions::{
            Action, error::{ActionError, ActionResult}, handler::ActionHandler, receipt::ActionReceipt,
        },
    },
    operations::{
        indexing::{IndexMode, job::IndexerJob},
    },
    register_action_handler,
    shared::types::SdPath,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use async_trait::async_trait;
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
    async fn validate(
        &self,
        _context: Arc<CoreContext>,
        action: &Action,
    ) -> ActionResult<()> {
        if let Action::LocationAdd { library_id: _, action } = action {
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
    ) -> ActionResult<ActionReceipt> {
        if let Action::LocationAdd { library_id, action } = action {
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
            
            let location_mode = match action.mode {
                IndexMode::Shallow => crate::location::IndexMode::Shallow,
                IndexMode::Content => crate::location::IndexMode::Content,
                IndexMode::Deep => crate::location::IndexMode::Deep,
            };
            
            let (location_id, location_name) = location_manager
                .add_location(library.clone(), action.path.clone(), action.name, device_record.id, location_mode)
                .await
                .map_err(|e| ActionError::Internal(e.to_string()))?;

            // Now dispatch an indexing job based on the mode
            let job_handle = {
                // Use the action mode directly since it's already the correct IndexMode

                // Create indexer job directly
                let job = IndexerJob::from_location(location_id, SdPath::local(action.path.clone()), action.mode);

                // Dispatch the job directly
                let job_handle = library
                    .jobs()
                    .dispatch(job)
                    .await
                    .map_err(ActionError::Job)?;

                Some(job_handle)
            };

            Ok(ActionReceipt::hybrid(
                Uuid::new_v4(),
                Some(serde_json::json!({
                    "location_id": location_id,
                    "name": location_name,
                    "path": action.path.display().to_string()
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