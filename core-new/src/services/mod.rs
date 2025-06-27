//! Background services management

use crate::{context::CoreContext, infrastructure::events::EventBus};
use anyhow::Result;
use std::sync::Arc;
use tracing::info;

pub mod location_watcher;
pub mod file_sharing;
pub mod device;

use location_watcher::{LocationWatcher, LocationWatcherConfig};
use file_sharing::FileSharingService;
use device::DeviceService;

/// Container for all background services
pub struct Services {
    /// File system watcher for locations
    pub location_watcher: Arc<LocationWatcher>,
    /// File sharing service
    pub file_sharing: Arc<FileSharingService>,
    /// Device management service
    pub device: Arc<DeviceService>,
    /// Shared context for all services
    context: Arc<CoreContext>,
}

impl Services {
    /// Create new services container with context
    pub fn new(context: Arc<CoreContext>) -> Self {
        info!("Initializing background services");
        
        let location_watcher_config = LocationWatcherConfig::default();
        let location_watcher = Arc::new(LocationWatcher::new(location_watcher_config, context.events.clone()));
        let file_sharing = Arc::new(FileSharingService::new(context.clone()));
        let device = Arc::new(DeviceService::new(context.clone()));
        
        Self {
            location_watcher,
            file_sharing,
            device,
            context,
        }
    }

    /// Get the shared context
    pub fn context(&self) -> Arc<CoreContext> {
        self.context.clone()
    }
    
    /// Start all services
    pub async fn start_all(&self) -> Result<()> {
        info!("Starting all background services");
        
        self.location_watcher.start().await?;
        
        // TODO: Start other services
        // self.jobs.start().await?;
        // self.thumbnails.start().await?;
        
        Ok(())
    }
    
    /// Stop all services gracefully
    pub async fn stop_all(&self) -> Result<()> {
        info!("Stopping all background services");
        
        self.location_watcher.stop().await?;
        
        // TODO: Stop other services
        // self.jobs.stop().await?;
        // self.thumbnails.stop().await?;
        
        Ok(())
    }
}

/// Trait for background services
#[async_trait::async_trait]
pub trait Service: Send + Sync {
    /// Start the service
    async fn start(&self) -> Result<()>;
    
    /// Stop the service gracefully
    async fn stop(&self) -> Result<()>;
    
    /// Check if the service is running
    fn is_running(&self) -> bool;
    
    /// Get service name for logging
    fn name(&self) -> &'static str;
}