//! Location rescan action handler

use crate::{
    context::CoreContext,
    infra::{
        action::{
            error::{ActionError, ActionResult},
            handler::ActionHandler,
            output::ActionOutput,
            Action,
        },
        db::entities,
    },
    ops::indexing::{IndexMode, job::IndexerJob, PathResolver},
    register_action_handler,
    domain::addressing::SdPath,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationRescanAction {
    pub location_id: Uuid,
    pub full_rescan: bool,
}

pub struct LocationRescanHandler;

impl LocationRescanHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ActionHandler for LocationRescanHandler {
    async fn execute(
        &self,
        context: Arc<CoreContext>,
        action: Action,
    ) -> ActionResult<ActionOutput> {
        if let Action::LocationRescan { library_id, action } = action {
            let library_manager = &context.library_manager;

            // Get the specific library
            let library = library_manager
                .get_library(library_id)
                .await
                .ok_or(ActionError::LibraryNotFound(library_id))?;

            // Get location details from database
            use crate::infra::db::entities;
            use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

            let location = entities::location::Entity::find()
                .filter(entities::location::Column::Uuid.eq(action.location_id))
                .one(library.db().conn())
                .await
                .map_err(|e| ActionError::Internal(format!("Database error: {}", e)))?
                .ok_or_else(|| ActionError::Internal(format!("Location not found: {}", action.location_id)))?;

            // Get the location's path using PathResolver
            let location_path_buf = PathResolver::get_full_path(library.db().conn(), location.entry_id)
                .await
                .map_err(|e| ActionError::Internal(format!("Failed to get location path: {}", e)))?;
            let location_path_str = location_path_buf.to_string_lossy().to_string();
            let location_path = SdPath::local(location_path_buf);

            // Determine index mode based on full_rescan flag
            let mode = if action.full_rescan {
                IndexMode::Deep
            } else {
                // Convert from string to IndexMode
                match location.index_mode.as_str() {
                    "shallow" => IndexMode::Shallow,
                    "content" => IndexMode::Content,
                    "deep" => IndexMode::Deep,
                    _ => IndexMode::Content,
                }
            };

            // Create indexer job for rescan
            let job = IndexerJob::from_location(action.location_id, location_path, mode);

            // Dispatch the job
            let job_handle = library
                .jobs()
                .dispatch(job)
                .await
                .map_err(ActionError::Job)?;

            let output = super::output::LocationRescanOutput {
                location_id: action.location_id,
                location_path: location_path_str,
                job_id: job_handle.id().into(),
                full_rescan: action.full_rescan,
            };

            Ok(ActionOutput::from_trait(output))
        } else {
            Err(ActionError::InvalidActionType)
        }
    }

    fn can_handle(&self, action: &Action) -> bool {
        matches!(action, Action::LocationRescan { .. })
    }

    fn supported_actions() -> &'static [&'static str] {
        &["location.rescan"]
    }
}

register_action_handler!(LocationRescanHandler, "location.rescan");