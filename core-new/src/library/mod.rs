//! Library management system
//! 
//! This module provides the core library functionality for Spacedrive.
//! Each library is a self-contained directory with its own database,
//! thumbnails, and other data.

mod config;
mod error;
mod lock;
mod manager;

pub use config::{LibraryConfig, LibrarySettings, LibraryStatistics};
pub use error::{LibraryError, Result};
pub use lock::LibraryLock;
pub use manager::{LibraryManager, DiscoveredLibrary};

use crate::infrastructure::{
    database::Database,
    jobs::manager::JobManager,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Represents an open Spacedrive library
pub struct Library {
    /// Root directory of the library (the .sdlibrary folder)
    path: PathBuf,
    
    /// Library configuration
    config: RwLock<LibraryConfig>,
    
    /// Database connection
    db: Arc<Database>,
    
    /// Job manager for this library
    jobs: Arc<JobManager>,
    
    /// Lock preventing concurrent access
    _lock: LibraryLock,
}

impl Library {
    /// Get the library ID
    pub fn id(&self) -> Uuid {
        // Config is immutable for ID, so we can use try_read
        self.config.try_read().map(|c| c.id).unwrap_or_else(|_| {
            // This should never happen in practice
            panic!("Failed to read library config for ID")
        })
    }
    
    /// Get the library name
    pub async fn name(&self) -> String {
        self.config.read().await.name.clone()
    }
    
    /// Get the library path
    pub fn path(&self) -> &Path {
        &self.path
    }
    
    /// Get the database
    pub fn db(&self) -> &Arc<Database> {
        &self.db
    }
    
    /// Get the job manager
    pub fn jobs(&self) -> &Arc<JobManager> {
        &self.jobs
    }
    
    /// Get a copy of the current configuration
    pub async fn config(&self) -> LibraryConfig {
        self.config.read().await.clone()
    }
    
    /// Update library configuration
    pub async fn update_config<F>(&self, f: F) -> Result<()>
    where
        F: FnOnce(&mut LibraryConfig),
    {
        let mut config = self.config.write().await;
        f(&mut config);
        config.updated_at = chrono::Utc::now();
        
        // Save to disk
        let config_path = self.path.join("library.json");
        let json = serde_json::to_string_pretty(&*config)?;
        tokio::fs::write(config_path, json).await?;
        
        Ok(())
    }
    
    /// Get the thumbnail directory for this library
    pub fn thumbnails_dir(&self) -> PathBuf {
        self.path.join("thumbnails")
    }
    
    /// Get the path for a specific thumbnail
    pub fn thumbnail_path(&self, cas_id: &str) -> PathBuf {
        if cas_id.len() < 2 {
            // Fallback for short IDs
            return self.thumbnails_dir().join(format!("{}.webp", cas_id));
        }
        
        // Two-level sharding based on first two characters
        let first = &cas_id[0..1];
        let second = &cas_id[1..2];
        
        self.thumbnails_dir()
            .join(first)
            .join(second)
            .join(format!("{}.webp", cas_id))
    }
    
    /// Save a thumbnail
    pub async fn save_thumbnail(&self, cas_id: &str, data: &[u8]) -> Result<()> {
        let path = self.thumbnail_path(cas_id);
        
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        
        // Write thumbnail
        tokio::fs::write(path, data).await?;
        
        Ok(())
    }
    
    /// Check if a thumbnail exists
    pub async fn has_thumbnail(&self, cas_id: &str) -> bool {
        tokio::fs::metadata(self.thumbnail_path(cas_id))
            .await
            .is_ok()
    }
    
    /// Get thumbnail data
    pub async fn get_thumbnail(&self, cas_id: &str) -> Result<Vec<u8>> {
        let path = self.thumbnail_path(cas_id);
        Ok(tokio::fs::read(path).await?)
    }
    
    /// Update library statistics
    pub async fn update_statistics<F>(&self, f: F) -> Result<()>
    where
        F: FnOnce(&mut LibraryStatistics),
    {
        self.update_config(|config| {
            f(&mut config.statistics);
            config.statistics.updated_at = chrono::Utc::now();
        }).await
    }
}

/// Current library configuration version
pub const LIBRARY_CONFIG_VERSION: u32 = 2;

/// Library directory extension
pub const LIBRARY_EXTENSION: &str = "sdlibrary";