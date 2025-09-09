//! Location remove action handler

use crate::{
    context::CoreContext,
    location::manager::LocationManager,
    infra::action::{
        error::ActionError,
        LibraryAction,
    },
};
use super::output::LocationRemoveOutput;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationRemoveAction {
    pub library_id: Uuid,
    pub location_id: Uuid,
}

impl LocationRemoveAction {
    /// Create a new location remove action
    pub fn new(library_id: Uuid, location_id: Uuid) -> Self {
        Self {
            library_id,
            location_id,
        }
    }
}

// Implement the unified LibraryAction (replaces ActionHandler)
impl LibraryAction for LocationRemoveAction {
    type Output = LocationRemoveOutput;

    async fn execute(self, library: std::sync::Arc<crate::library::Library>, context: Arc<CoreContext>) -> Result<Self::Output, ActionError> {

        // Remove the location
        let location_manager = LocationManager::new(context.events.as_ref().clone());
        location_manager
            .remove_location(&library, self.location_id)
            .await
            .map_err(|e| ActionError::Internal(e.to_string()))?;

        Ok(LocationRemoveOutput::new(self.location_id, None))
    }

    fn action_kind(&self) -> &'static str {
        "location.remove"
    }

    fn library_id(&self) -> Uuid {
        self.library_id
    }

    async fn validate(&self, library: &std::sync::Arc<crate::library::Library>, context: Arc<CoreContext>) -> Result<(), ActionError> {
        // Could add validation to check if location exists in the library
        Ok(())
    }
}
