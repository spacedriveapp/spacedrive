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

use crate::infra::{
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

    /// Save library configuration to disk
    pub async fn save_config(&self, config: &LibraryConfig) -> Result<()> {
        let config_path = self.path.join("library.json");
        let json = serde_json::to_string_pretty(config)?;
        tokio::fs::write(config_path, json).await?;
        Ok(())
    }

    /// Get the thumbnail directory for this library
    pub fn thumbnails_dir(&self) -> PathBuf {
        self.path.join("thumbnails")
    }

    /// Get the path for a specific thumbnail with size
    pub fn thumbnail_path(&self, cas_id: &str, size: u32) -> PathBuf {
        if cas_id.len() < 4 {
            // Fallback for short IDs
            return self.thumbnails_dir().join(format!("{}_{}.webp", cas_id, size));
        }

        // Two-level sharding based on first four characters
        let shard1 = &cas_id[0..2];
        let shard2 = &cas_id[2..4];

        self.thumbnails_dir()
            .join(shard1)
            .join(shard2)
            .join(format!("{}_{}.webp", cas_id, size))
    }

    /// Get the path for any thumbnail size (legacy compatibility)
    pub fn thumbnail_path_legacy(&self, cas_id: &str) -> PathBuf {
        self.thumbnail_path(cas_id, 256) // Default to 256px
    }

    /// Save a thumbnail with specific size
    pub async fn save_thumbnail(&self, cas_id: &str, size: u32, data: &[u8]) -> Result<()> {
        let path = self.thumbnail_path(cas_id, size);

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Write thumbnail
        tokio::fs::write(path, data).await?;

        Ok(())
    }

    /// Check if a thumbnail exists for a specific size
    pub async fn has_thumbnail(&self, cas_id: &str, size: u32) -> bool {
        tokio::fs::metadata(self.thumbnail_path(cas_id, size))
            .await
            .is_ok()
    }

    /// Shutdown the library, gracefully stopping all jobs
    pub async fn shutdown(&self) -> Result<()> {
        // Shutdown the job manager, which will pause all running jobs
        self.jobs.shutdown().await?;

        // Save config to ensure any updates are persisted
        let config = self.config.read().await;
        self.save_config(&*config).await?;

        Ok(())
    }

    /// Check if thumbnails exist for all specified sizes
    pub async fn has_all_thumbnails(&self, cas_id: &str, sizes: &[u32]) -> bool {
        for &size in sizes {
            if !self.has_thumbnail(cas_id, size).await {
                return false;
            }
        }
        true
    }

    /// Get thumbnail data for specific size
    pub async fn get_thumbnail(&self, cas_id: &str, size: u32) -> Result<Vec<u8>> {
        let path = self.thumbnail_path(cas_id, size);
        Ok(tokio::fs::read(path).await?)
    }

    /// Get the best available thumbnail (largest size available)
    pub async fn get_best_thumbnail(&self, cas_id: &str, preferred_sizes: &[u32]) -> Result<Option<(u32, Vec<u8>)>> {
        // Try sizes in descending order
        let mut sizes = preferred_sizes.to_vec();
        sizes.sort_by(|a, b| b.cmp(a));

        for &size in &sizes {
            if self.has_thumbnail(cas_id, size).await {
                let data = self.get_thumbnail(cas_id, size).await?;
                return Ok(Some((size, data)));
            }
        }

        Ok(None)
    }

    /// Start thumbnail generation job
    pub async fn generate_thumbnails(&self, entry_ids: Option<Vec<Uuid>>) -> Result<crate::infra::jobs::handle::JobHandle> {
        use crate::ops::media::thumbnail::{ThumbnailJob, ThumbnailJobConfig};

        let config = ThumbnailJobConfig {
            sizes: self.config().await.settings.thumbnail_sizes.clone(),
            quality: self.config().await.settings.thumbnail_quality,
            regenerate: false,
            batch_size: 50,
            max_concurrent: 4,
        };

        let job = if let Some(ids) = entry_ids {
            ThumbnailJob::for_entries(ids, config)
        } else {
            ThumbnailJob::new(config)
        };

        self.jobs().dispatch(job).await
            .map_err(|e| LibraryError::JobError(e))
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

// Note: Library does not implement Clone due to the exclusive lock
// Use Arc<Library> when you need shared access

/// Current library configuration version
pub const LIBRARY_CONFIG_VERSION: u32 = 2;

/// Library directory extension
pub const LIBRARY_EXTENSION: &str = "sdlibrary";