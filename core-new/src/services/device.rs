//! Device management service
//! 
//! Provides access to device connection information and networking functionality

use crate::{context::CoreContext, services::networking};
use anyhow::Result;
use std::sync::Arc;
use uuid::Uuid;

/// Service for managing device connections and information
pub struct DeviceService {
    context: Arc<CoreContext>,
}

impl DeviceService {
    /// Create a new device service
    pub fn new(context: Arc<CoreContext>) -> Self {
        Self { context }
    }

    /// Get list of connected device IDs
    pub async fn get_connected_devices(&self) -> Result<Vec<Uuid>> {
        if let Some(networking) = self.context.get_networking().await {
            let service = &*networking;
            let devices = service.get_connected_devices().await;
            Ok(devices.into_iter().map(|d| d.device_id).collect())
        } else {
            Ok(Vec::new())
        }
    }

    /// Get detailed information about connected devices
    pub async fn get_connected_devices_info(&self) -> Result<Vec<networking::DeviceInfo>> {
        if let Some(networking) = self.context.get_networking().await {
            let service = &*networking;
            let devices = service.get_connected_devices().await;
            Ok(devices)
        } else {
            Ok(Vec::new())
        }
    }
}