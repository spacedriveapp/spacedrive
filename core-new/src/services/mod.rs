//! Background services management

use crate::infrastructure::events::EventBus;
use anyhow::Result;
use std::sync::Arc;
use tracing::info;

pub mod location_watcher;

use location_watcher::{LocationWatcher, LocationWatcherConfig};

/// Container for all background services
pub struct Services {
    /// File system watcher for locations
    pub location_watcher: Arc<LocationWatcher>,
    // TODO: Add other services when implemented
    // pub jobs: Arc<JobManager>,
    // pub thumbnails: Arc<ThumbnailService>,
    // pub sync: Arc<SyncService>,
    // pub p2p: Arc<P2PService>,
}

impl Services {
    /// Create new services container
    pub fn new(events: Arc<EventBus>) -> Self {
        info!("Initializing background services");
        
        let location_watcher_config = LocationWatcherConfig::default();
        let location_watcher = Arc::new(LocationWatcher::new(location_watcher_config, events));
        
        // TODO: Initialize other services
        // let jobs = Arc::new(JobManager::new());
        // let thumbnails = Arc::new(ThumbnailService::new());
        
        Self {
            location_watcher,
            // jobs,
            // thumbnails,
        }
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