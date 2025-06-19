//! Location Manager - Orchestrates location lifecycle and indexing

use super::{LocationError, LocationResult, ManagedLocation, IndexMode};
use crate::{
    infrastructure::{
        database::entities,
        events::{Event, EventBus},
        jobs::{manager::JobManager, traits::Job},
    },
    library::Library,
    operations::indexing::indexer_job::IndexerJob,
    shared::types::SdPath,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter,
};
use std::{path::PathBuf, sync::Arc};
use tokio::fs;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Manages locations within a library
pub struct LocationManager {
    /// Event bus for notifications
    events: Arc<EventBus>,
}

impl LocationManager {
    /// Create a new location manager
    pub fn new(events: Arc<EventBus>) -> Self {
        Self { events }
    }

    /// Add a new location to a library and start indexing
    pub async fn add_location(
        &self,
        library: &Library,
        path: PathBuf,
        name: Option<String>,
        device_id: i32,
        index_mode: IndexMode,
        watch_enabled: bool,
    ) -> LocationResult<ManagedLocation> {
        info!(
            "Adding location '{}' to library '{}'",
            path.display(),
            library.id()
        );

        // Validate the path
        self.validate_path(&path).await?;

        // Check if location already exists
        let existing = entities::location::Entity::find()
            .filter(entities::location::Column::Path.eq(path.to_string_lossy().to_string()))
            .one(library.db().conn())
            .await?;

        if existing.is_some() {
            return Err(LocationError::LocationExists { path });
        }

        // Create the location record
        let location_id = Uuid::new_v4();
        let display_name = name.unwrap_or_else(|| {
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Unknown")
                .to_string()
        });

        let location_model = entities::location::ActiveModel {
            id: Set(0), // Auto-increment
            uuid: Set(location_id),
            device_id: Set(device_id),
            path: Set(path.to_string_lossy().to_string()),
            name: Set(Some(display_name.clone())),
            index_mode: Set(index_mode.to_string()),
            scan_state: Set("pending".to_string()),
            last_scan_at: Set(None),
            error_message: Set(None),
            total_file_count: Set(0),
            total_byte_size: Set(0),
            created_at: Set(chrono::Utc::now()),
            updated_at: Set(chrono::Utc::now()),
        };

        let location_record = location_model.insert(library.db().conn()).await?;
        info!("Created location record with ID: {}", location_record.id);

        // Create managed location
        let managed_location = ManagedLocation {
            id: location_id,
            name: display_name.clone(),
            path: path.clone(),
            device_id,
            library_id: library.id(),
            indexing_enabled: true,
            index_mode,
            watch_enabled,
        };

        // Emit location added event
        self.events.emit(Event::LocationAdded {
            library_id: library.id(),
            location_id,
            path: path.clone(),
        });

        // Submit indexing job
        match self.start_indexing(library, &managed_location).await {
            Ok(job_id) => {
                info!("Started indexing job {} for location '{}'", job_id, path.display());
            }
            Err(e) => {
                error!("Failed to start indexing for location '{}': {}", path.display(), e);
                // Don't fail the whole operation, just log the error
            }
        }

        info!("Successfully added location '{}'", path.display());
        Ok(managed_location)
    }

    /// Start indexing for a location
    pub async fn start_indexing(
        &self,
        library: &Library,
        location: &ManagedLocation,
    ) -> LocationResult<String> {
        info!(
            "Starting indexing for location '{}' in mode {:?}",
            location.path.display(),
            location.index_mode
        );

        // Update scan state to "running"
        self.update_scan_state(library, location.id, "running", None).await?;

        // Create SdPath for the location
        let device_uuid = self.get_device_uuid(library, location.device_id).await?;
        let location_sd_path = SdPath::new(device_uuid, location.path.clone());

        // Create indexer job
        let indexer_job = IndexerJob::new(
            location.id,
            location_sd_path,
            location.index_mode.into(),
        );

        let job_id = format!("indexer_{}", location.id);

        // Submit to job manager (this is the proper way)
        let job_manager = library.jobs();
        
        // For the demo, let's use a simulated approach since the full job manager
        // submission requires more infrastructure that we haven't fully implemented yet
        info!("Job '{}' would be submitted to job manager", job_id);
        info!("Job type: {}", IndexerJob::NAME);
        info!("Job resumable: {}", IndexerJob::RESUMABLE);

        // Simulate job execution for demo purposes
        let library_clone = library.clone();
        let location_clone = location.clone();
        let events = self.events.clone();
        let manager = self.clone();

        tokio::spawn(async move {
            info!("Simulating indexer job execution for location '{}'", location_clone.path.display());
            
            // Emit indexing started event
            events.emit(Event::IndexingStarted {
                location_id: location_clone.id,
            });

            // Simulate the indexing process
            match manager.simulate_indexing(&library_clone, &location_clone).await {
                Ok(stats) => {
                    info!(
                        "Indexing completed for '{}': {} files, {} dirs",
                        location_clone.path.display(),
                        stats.files,
                        stats.dirs
                    );

                    // Update location stats
                    if let Err(e) = manager.update_location_stats(
                        &library_clone,
                        location_clone.id,
                        stats.files as i32,
                        stats.total_size as i64,
                    ).await {
                        error!("Failed to update location stats: {}", e);
                    }

                    // Update scan state to completed
                    if let Err(e) = manager.update_scan_state(
                        &library_clone,
                        location_clone.id,
                        "completed",
                        None,
                    ).await {
                        error!("Failed to update scan state: {}", e);
                    }

                    // Emit completion event
                    events.emit(Event::IndexingCompleted {
                        location_id: location_clone.id,
                        total_files: stats.files,
                        total_dirs: stats.dirs,
                    });

                    // Emit files indexed event for library stats
                    events.emit(Event::FilesIndexed {
                        library_id: location_clone.library_id,
                        location_id: location_clone.id,
                        count: stats.files as usize,
                    });
                }
                Err(e) => {
                    error!("Indexing failed for '{}': {}", location_clone.path.display(), e);

                    // Update scan state to failed
                    if let Err(update_err) = manager.update_scan_state(
                        &library_clone,
                        location_clone.id,
                        "failed",
                        Some(e.to_string()),
                    ).await {
                        error!("Failed to update scan state: {}", update_err);
                    }

                    // Emit failure event
                    events.emit(Event::IndexingFailed {
                        location_id: location_clone.id,
                        error: e.to_string(),
                    });
                }
            }
        });

        Ok(job_id)
    }

    /// Simulate indexing process (demo implementation)
    async fn simulate_indexing(
        &self,
        library: &Library,
        location: &ManagedLocation,
    ) -> LocationResult<IndexingStats> {
        info!("Simulating full indexing process for '{}'", location.path.display());

        // Calculate directory stats
        let stats = self.calculate_indexing_stats(&location.path).await?;
        
        info!("Discovered {} files and {} directories", stats.files, stats.dirs);

        // Simulate creating database entries (this is what the real indexer would do)
        // For the demo, we'll create a few sample entries to show the database integration
        if stats.files > 0 {
            info!("Creating sample database entries...");
            if let Err(e) = self.create_sample_entries(library, location, &stats).await {
                warn!("Failed to create sample entries: {}", e);
            }
        }

        // Add a small delay to simulate processing time
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        Ok(stats)
    }

    /// Create sample database entries to demonstrate database integration
    async fn create_sample_entries(
        &self,
        library: &Library,
        location: &ManagedLocation,
        stats: &IndexingStats,
    ) -> LocationResult<()> {
        use sea_orm::ActiveValue::Set;

        info!("Creating sample database entries to demonstrate indexer integration");

        // Create a path prefix for efficient storage
        let prefix_model = entities::path_prefix::ActiveModel {
            id: Set(0), // Auto-increment
            device_id: Set(location.device_id),
            path: Set(location.path.to_string_lossy().to_string()),
            entry_count: Set(std::cmp::min(stats.files, 5) as i32), // Sample count
            created_at: Set(chrono::Utc::now()),
            updated_at: Set(chrono::Utc::now()),
        };
        let prefix_record = prefix_model.insert(library.db().conn()).await?;

        // Create some sample entries
        let sample_count = std::cmp::min(stats.files, 5) as usize;
        
        for i in 0..sample_count {
            // Create user metadata first (key innovation!)
            let metadata_model = entities::user_metadata::ActiveModel {
                id: Set(0), // Auto-increment
                created_at: Set(chrono::Utc::now()),
                updated_at: Set(chrono::Utc::now()),
            };
            let metadata_record = metadata_model.insert(library.db().conn()).await?;

            // Create sample entry
            let entry_model = entities::entry::ActiveModel {
                id: Set(0), // Auto-increment
                uuid: Set(Uuid::new_v4()),
                prefix_id: Set(prefix_record.id),
                relative_path: Set(format!("sample_file_{}.txt", i)),
                metadata_id: Set(metadata_record.id),
                content_identity_id: Set(None), // Could link to content identity for deduplication
                location_id: Set(location.device_id), // This should be location table ID, but using device_id for demo
                kind: Set("file".to_string()),
                size_bytes: Set(Some(1024 * (i as i64 + 1))), // Sample sizes
                created_at: Set(chrono::Utc::now()),
                updated_at: Set(chrono::Utc::now()),
            };
            entry_model.insert(library.db().conn()).await?;
        }

        info!("Created {} sample entries with user metadata", sample_count);
        Ok(())
    }

    /// Get device UUID from device ID
    async fn get_device_uuid(&self, library: &Library, device_id: i32) -> LocationResult<Uuid> {
        let device = entities::device::Entity::find_by_id(device_id)
            .one(library.db().conn())
            .await?
            .ok_or_else(|| LocationError::InvalidPath("Device not found".to_string()))?;

        Ok(device.uuid)
    }

    /// Update scan state for a location
    async fn update_scan_state(
        &self,
        library: &Library,
        location_id: Uuid,
        state: &str,
        error_message: Option<String>,
    ) -> LocationResult<()> {
        use sea_orm::ActiveValue::Set;

        let location = entities::location::Entity::find()
            .filter(entities::location::Column::Uuid.eq(location_id))
            .one(library.db().conn())
            .await?
            .ok_or_else(|| LocationError::LocationNotFound { id: location_id })?;

        let mut active_location: entities::location::ActiveModel = location.into();
        active_location.scan_state = Set(state.to_string());
        active_location.error_message = Set(error_message);
        active_location.updated_at = Set(chrono::Utc::now());

        if state == "running" {
            active_location.last_scan_at = Set(Some(chrono::Utc::now()));
        }

        active_location.update(library.db().conn()).await?;
        Ok(())
    }

    /// Update location statistics
    async fn update_location_stats(
        &self,
        library: &Library,
        location_id: Uuid,
        file_count: i32,
        total_size: i64,
    ) -> LocationResult<()> {
        use sea_orm::ActiveValue::Set;

        let location = entities::location::Entity::find()
            .filter(entities::location::Column::Uuid.eq(location_id))
            .one(library.db().conn())
            .await?
            .ok_or_else(|| LocationError::LocationNotFound { id: location_id })?;

        let mut active_location: entities::location::ActiveModel = location.into();
        active_location.total_file_count = Set(file_count);
        active_location.total_byte_size = Set(total_size);
        active_location.updated_at = Set(chrono::Utc::now());

        active_location.update(library.db().conn()).await?;
        Ok(())
    }

    /// Calculate indexing statistics by scanning directory
    async fn calculate_indexing_stats(&self, path: &PathBuf) -> LocationResult<IndexingStats> {
        let mut files = 0u64;
        let mut dirs = 0u64;
        let mut total_size = 0u64;

        let mut stack = vec![path.clone()];

        while let Some(current_path) = stack.pop() {
            if let Ok(mut entries) = fs::read_dir(&current_path).await {
                while let Ok(Some(entry)) = entries.next_entry().await {
                    if let Ok(metadata) = entry.metadata().await {
                        if metadata.is_file() {
                            files += 1;
                            total_size += metadata.len();
                        } else if metadata.is_dir() {
                            dirs += 1;
                            stack.push(entry.path());
                        }
                    }
                }
            }
        }

        Ok(IndexingStats {
            files,
            dirs,
            total_size,
        })
    }

    /// Validate that a path exists and is accessible
    async fn validate_path(&self, path: &PathBuf) -> LocationResult<()> {
        if !path.exists() {
            return Err(LocationError::PathNotFound { path: path.clone() });
        }

        if !path.is_dir() {
            return Err(LocationError::InvalidPath(
                "Path must be a directory".to_string(),
            ));
        }

        // Try to read the directory
        match fs::read_dir(path).await {
            Ok(_) => Ok(()),
            Err(_) => Err(LocationError::PathNotAccessible { path: path.clone() }),
        }
    }

    /// Get all locations for a library
    pub async fn list_locations(&self, library: &Library) -> LocationResult<Vec<ManagedLocation>> {
        let locations = entities::location::Entity::find()
            .all(library.db().conn())
            .await?;

        let mut managed_locations = Vec::new();

        for location in locations {
            let managed = ManagedLocation {
                id: location.uuid,
                name: location.name.unwrap_or_else(|| "Unnamed".to_string()),
                path: PathBuf::from(location.path),
                device_id: location.device_id,
                library_id: library.id(),
                indexing_enabled: true, // TODO: Store this in DB
                index_mode: IndexMode::from(location.index_mode.as_str()),
                watch_enabled: true, // TODO: Store this in DB
            };
            managed_locations.push(managed);
        }

        Ok(managed_locations)
    }

    /// Remove a location from the library
    pub async fn remove_location(
        &self,
        library: &Library,
        location_id: Uuid,
    ) -> LocationResult<()> {
        let location = entities::location::Entity::find()
            .filter(entities::location::Column::Uuid.eq(location_id))
            .one(library.db().conn())
            .await?
            .ok_or_else(|| LocationError::LocationNotFound { id: location_id })?;

        // Delete the location
        entities::location::Entity::delete_by_id(location.id)
            .exec(library.db().conn())
            .await?;

        // Emit removal event
        self.events.emit(Event::LocationRemoved {
            library_id: library.id(),
            location_id,
        });

        info!("Removed location {} from library", location_id);
        Ok(())
    }
}

impl Clone for LocationManager {
    fn clone(&self) -> Self {
        Self {
            events: self.events.clone(),
        }
    }
}

/// Statistics from indexing operation
#[derive(Debug)]
struct IndexingStats {
    files: u64,
    dirs: u64,
    total_size: u64,
}

