//! Device revoke action handler

use crate::{
    context::CoreContext,
    infrastructure::actions::{
        error::{ActionError, ActionResult},
        handler::ActionHandler,
        output::ActionOutput,
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
    pub device_id: Uuid,
    pub reason: Option<String>,
}

pub struct DeviceRevokeHandler;

impl DeviceRevokeHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ActionHandler for DeviceRevokeHandler {
    async fn validate(
        &self,
        context: Arc<CoreContext>,
        action: &Action,
    ) -> ActionResult<()> {
        if let Action::DeviceRevoke { action, .. } = action {
            // Don't allow revoking self
            let current_device = context.device_manager.to_device()
                .map_err(|e| ActionError::Internal(format!("Failed to get current device: {}", e)))?;
            
            if current_device.id == action.device_id {
                return Err(ActionError::Validation {
                    field: "device_id".to_string(),
                    message: "Cannot revoke current device".to_string(),
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
    ) -> ActionResult<ActionOutput> {
        if let Action::DeviceRevoke { library_id, action } = action {
            let library_manager = &context.library_manager;
            
            // Get the specific library
            let library = library_manager
                .get_library(library_id)
                .await
                .ok_or(ActionError::LibraryNotFound(library_id))?;

            // Remove device from database
            use crate::infrastructure::database::entities;
            use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, ModelTrait};
            
            let device = entities::device::Entity::find()
                .filter(entities::device::Column::Uuid.eq(action.device_id))
                .one(library.db().conn())
                .await
                .map_err(|e| ActionError::Internal(format!("Database error: {}", e)))?
                .ok_or_else(|| ActionError::Internal(format!("Device not found: {}", action.device_id)))?;

            let device_name = device.name.clone();
            
            // Delete the device
            device.delete(library.db().conn())
                .await
                .map_err(|e| ActionError::Internal(format!("Failed to delete device: {}", e)))?;

            // TODO: Also revoke any active network connections for this device
            // This would involve the networking/P2P system

            let output = super::output::DeviceRevokeOutput {
                device_id: action.device_id,
                device_name,
                reason: action.reason,
            };

            Ok(ActionOutput::from_trait(output))
        } else {
            Err(ActionError::InvalidActionType)
        }
    }

    fn can_handle(&self, action: &Action) -> bool {
        matches!(action, Action::DeviceRevoke { .. })
    }

    fn supported_actions() -> &'static [&'static str] {
        &["device.revoke"]
    }
}

register_action_handler!(DeviceRevokeHandler, "device.revoke");