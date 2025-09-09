//! Device revoke action handler

use crate::{
    context::CoreContext,
    infra::action::{
        error::{ActionError, ActionResult},
        Action,
    },
    register_action_handler,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceRevokeAction {
    pub library_id: Uuid,
    pub device_id: Uuid,
    pub reason: Option<String>,
}

impl DeviceRevokeAction {
    /// Create a new device revoke action
    pub fn new(library_id: Uuid, device_id: Uuid, reason: Option<String>) -> Self {
        Self {
            library_id,
            device_id,
            reason,
        }
    }
}

pub struct DeviceRevokeHandler;

impl DeviceRevokeHandler {
    pub fn new() -> Self {
        Self
    }
}

// Old ActionHandler implementation removed - using unified LibraryAction

// Implement the unified LibraryAction (replaces ActionHandler)
impl LibraryAction for DeviceRevokeAction {
    type Output = super::output::DeviceRevokeOutput;

    async fn execute(self, library: std::sync::Arc<crate::library::Library>, context: Arc<CoreContext>) -> Result<Self::Output, ActionError> {
        // Library is pre-validated by ActionManager

        // TODO: Implement device revocation logic
        let device_name = format!("Device {}", self.device_id);

        Ok(super::output::DeviceRevokeOutput {
            device_id: self.device_id,
            device_name,
            reason: self.reason.unwrap_or_else(|| "No reason provided".to_string()),
        })
    }

    fn action_kind(&self) -> &'static str {
        "device.revoke"
    }

    fn library_id(&self) -> Uuid {
        self.library_id
    }

    async fn validate(&self, library: &std::sync::Arc<crate::library::Library>, context: Arc<CoreContext>) -> Result<(), ActionError> {
        // Don't allow revoking self
        let current_device = context.device_manager.to_device()
            .map_err(|e| ActionError::Internal(format!("Failed to get current device: {}", e)))?;

        if current_device.id == self.device_id {
            return Err(ActionError::Validation {
                field: "device_id".to_string(),
                message: "Cannot revoke current device".to_string(),
            });
        }
        Ok(())
    }
}

// All old ActionHandler code removed