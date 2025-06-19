//! Background services management

use anyhow::Result;
use std::sync::Arc;
use tracing::info;

/// Container for all background services
pub struct Services {
    // TODO: Add actual services when implemented
    // pub locations: Arc<LocationWatcher>,
    // pub jobs: Arc<JobManager>,
    // pub thumbnails: Arc<ThumbnailService>,
    // pub sync: Arc<SyncService>,
    // pub p2p: Arc<P2PService>,
}

impl Services {
    /// Create new services container
    pub fn new() -> Self {
        info!("Initializing background services");
        
        // TODO: Initialize actual services
        // let locations = Arc::new(LocationWatcher::new());
        // let jobs = Arc::new(JobManager::new());
        // let thumbnails = Arc::new(ThumbnailService::new());
        
        Self {
            // locations,
            // jobs,
            // thumbnails,
        }
    }
    
    /// Stop all services gracefully
    pub async fn stop_all(&self) -> Result<()> {
        info!("Stopping all background services");
        
        // TODO: Stop actual services
        // self.locations.stop().await?;
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