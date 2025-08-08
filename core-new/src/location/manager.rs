//! Location Manager - Orchestrates location lifecycle and indexing

use super::{LocationError, LocationResult, ManagedLocation, IndexMode};
use crate::{
    infrastructure::{
        database::entities::{self, entry::EntryKind},
        events::{Event, EventBus},
        jobs::{manager::JobManager, traits::Job},
    },
    library::Library,
    operations::indexing::{job::{IndexerJob, IndexerJobConfig}, PathResolver},
    shared::types::SdPath,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, TransactionTrait,
};
use std::{path::PathBuf, sync::Arc};
use tokio::fs;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Manages locations and their lifecycle
#[derive(Clone)]
pub struct LocationManager {
    events: EventBus,
}

impl LocationManager {
    pub fn new(events: EventBus) -> Self {
        Self { events }
    }

    /// Add a new location to the library
    pub async fn add_location(
        &self,
        library: Arc<Library>,
        path: PathBuf,
        name: Option<String>,
        device_id: i32,
        index_mode: IndexMode,
    ) -> LocationResult<(Uuid, String)> {
        info!("Adding location: {}", path.display());

        // Validate the path
        self.validate_path(&path).await?;

        // Begin transaction
        let txn = library.db().conn().begin().await?;

        // Create entry for the location directory
        let directory_name = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown")
            .to_string();

        let entry_model = entities::entry::ActiveModel {
            uuid: Set(Some(Uuid::new_v4())),
            name: Set(directory_name.clone()),
            kind: Set(EntryKind::Directory as i32),
            extension: Set(None),
            metadata_id: Set(None),
            content_id: Set(None),
            size: Set(0),
            aggregate_size: Set(0),
            child_count: Set(0),
            file_count: Set(0),
            created_at: Set(chrono::Utc::now()),
            modified_at: Set(chrono::Utc::now()),
            accessed_at: Set(None),
            permissions: Set(None),
            inode: Set(None),
            parent_id: Set(None), // Location root has no parent
            ..Default::default()
        };

        let entry_record = entry_model.insert(&txn).await?;
        let entry_id = entry_record.id;

        // Add self-reference to closure table
        let self_closure = entities::entry_closure::ActiveModel {
            ancestor_id: Set(entry_id),
            descendant_id: Set(entry_id),
            depth: Set(0),
            ..Default::default()
        };
        self_closure.insert(&txn).await?;

        // Add to directory_paths table
        let dir_path_entry = entities::directory_paths::ActiveModel {
            entry_id: Set(entry_id),
            path: Set(path.to_string_lossy().to_string()),
            ..Default::default()
        };
        dir_path_entry.insert(&txn).await?;

        // Create the location record
        let location_id = Uuid::new_v4();
        let display_name = name.unwrap_or_else(|| {
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Unknown")
                .to_string()
        });

        let location_model = entities::location::ActiveModel {
            id: sea_orm::ActiveValue::NotSet,
            uuid: Set(location_id),
            device_id: Set(device_id),
            entry_id: Set(entry_id),
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

        let location_record = location_model.insert(&txn).await?;
        
        // Commit transaction
        txn.commit().await?;
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
            watch_enabled: true,
        };

        // Emit location added event
        self.events.emit(Event::LocationAdded {
            library_id: library.id(),
            location_id,
            path: path.clone(),
        });

        // Also emit indexing started event
        self.events.emit(Event::IndexingStarted { location_id });

        // Start indexing job
        let job_id = match self.start_indexing(library, &managed_location).await {
            Ok(job_id) => {
                info!("Started indexing job {} for location '{}'", job_id, path.display());

                // Emit job started event
                self.events.emit(Event::JobStarted {
                    job_id: job_id.clone(),
                    job_type: "Indexing".to_string(),
                });

                job_id
            }
            Err(e) => {
                error!("Failed to start indexing for location '{}': {}", path.display(), e);
                // Return empty job ID if indexing fails
                String::new()
            }
        };

        info!("Successfully added location '{}'", path.display());
        Ok((location_id, job_id))
    }

    /// Start indexing for a location
    pub async fn start_indexing(
        &self,
        library: Arc<Library>,
        location: &ManagedLocation,
    ) -> LocationResult<String> {
        info!(
            "Starting indexing for location '{}' in mode {:?}",
            location.path.display(),
            location.index_mode
        );

        // Update scan state to "scanning"
        self.update_scan_state(&library, location.id, "scanning", None).await?;

        // Create SdPath for the location
        let device_uuid = self.get_device_uuid(&library, location.device_id).await?;
        let location_sd_path = SdPath::new(device_uuid, location.path.clone());

        // Create indexer job using new configuration pattern
        let config = IndexerJobConfig::new(location.id, location_sd_path, location.index_mode.into());
        let indexer_job = IndexerJob::new(config);

        // Submit to job manager
        let job_manager = library.jobs();
        let job_handle = job_manager.dispatch(indexer_job).await?;
        let job_id = job_handle.id();

        info!("Started indexing job {} for location '{}'", job_id, location.path.display());

        // The job system will handle:
        // - Progress updates via the event bus
        // - Updating scan state when complete/failed
        // - Emitting appropriate events

        Ok(job_id.to_string())
    }

    /// Update scan state for a location
    async fn update_scan_state(
        &self,
        library: &Library,
        location_id: Uuid,
        scan_state: &str,
        error_message: Option<String>,
    ) -> LocationResult<()> {
        use sea_orm::ActiveValue::Set;

        let location = entities::location::Entity::find()
            .filter(entities::location::Column::Uuid.eq(location_id))
            .one(library.db().conn())
            .await?
            .ok_or_else(|| LocationError::LocationNotFound { id: location_id })?;

        let mut active_location: entities::location::ActiveModel = location.into();
        active_location.scan_state = Set(scan_state.to_string());
        active_location.error_message = Set(error_message);
        if scan_state == "completed" {
            active_location.last_scan_at = Set(Some(chrono::Utc::now()));
        }
        active_location.updated_at = Set(chrono::Utc::now());

        active_location.update(library.db().conn()).await?;
        Ok(())
    }

    /// Update location statistics
    pub async fn update_location_stats(
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
        active_location.total_file_count = Set(file_count as i64);
        active_location.total_byte_size = Set(total_size);
        active_location.updated_at = Set(chrono::Utc::now());

        active_location.update(library.db().conn()).await?;
        Ok(())
    }

    /// Get device UUID from device ID
    async fn get_device_uuid(&self, library: &Library, device_id: i32) -> LocationResult<Uuid> {
        let device = entities::device::Entity::find_by_id(device_id)
            .one(library.db().conn())
            .await?
            .ok_or_else(|| LocationError::Other(format!("Device {} not found", device_id)))?;

        Ok(device.uuid)
    }

    /// Validate a path before creating a location
    async fn validate_path(&self, path: &PathBuf) -> LocationResult<()> {
        // Check if path exists
        if !path.exists() {
            return Err(LocationError::PathNotFound {
                path: path.clone()
            });
        }

        // Check if it's a directory
        let metadata = fs::metadata(path).await?;
        if !metadata.is_dir() {
            return Err(LocationError::InvalidPath(
                "Path must be a directory".to_string()
            ));
        }

        // Check if we have read permissions
        match fs::read_dir(path).await {
            Ok(_) => Ok(()),
            Err(e) => match e.kind() {
                std::io::ErrorKind::PermissionDenied => {
                    Err(LocationError::PathNotAccessible {
                        path: path.clone()
                    })
                }
                _ => Err(LocationError::Io(e)),
            }
        }
    }

    /// Remove a location
    pub async fn remove_location(
        &self,
        library: &Library,
        location_id: Uuid,
    ) -> LocationResult<()> {
        info!("Removing location {}", location_id);

        // Find the location
        let location = entities::location::Entity::find()
            .filter(entities::location::Column::Uuid.eq(location_id))
            .one(library.db().conn())
            .await?
            .ok_or_else(|| LocationError::LocationNotFound { id: location_id })?;

        // Delete the location (cascades to entries)
        entities::location::Entity::delete_by_id(location.id)
            .exec(library.db().conn())
            .await?;

        // Emit event
        self.events.emit(Event::LocationRemoved {
            library_id: library.id(),
            location_id,
        });

        info!("Successfully removed location {}", location_id);
        Ok(())
    }

    /// List all locations for a library
    pub async fn list_locations(
        &self,
        library: &Library,
    ) -> LocationResult<Vec<ManagedLocation>> {
        let locations = entities::location::Entity::find()
            .all(library.db().conn())
            .await?;

        let mut managed_locations = Vec::new();
        for loc in locations {
            let path = PathResolver::get_full_path(library.db().conn(), loc.entry_id).await?;
            managed_locations.push(ManagedLocation {
                id: loc.uuid,
                name: loc.name.unwrap_or_else(|| "Unknown".to_string()),
                path,
                device_id: loc.device_id,
                library_id: library.id(),
                indexing_enabled: true,
                index_mode: loc.index_mode.parse().unwrap_or(IndexMode::Content),
                watch_enabled: true,
            });
        }

        Ok(managed_locations)
    }

    /// Rescan a location
    pub async fn rescan_location(
        &self,
        library: Arc<Library>,
        location_id: Uuid,
        force: bool,
    ) -> LocationResult<String> {
        info!("Rescanning location {} (force: {})", location_id, force);

        // Get the location
        let location = entities::location::Entity::find()
            .filter(entities::location::Column::Uuid.eq(location_id))
            .one(library.db().conn())
            .await?
            .ok_or_else(|| LocationError::LocationNotFound { id: location_id })?;

        let path = PathResolver::get_full_path(library.db().conn(), location.entry_id).await?;
        
        let managed_location = ManagedLocation {
            id: location.uuid,
            name: location.name.unwrap_or_else(|| "Unknown".to_string()),
            path,
            device_id: location.device_id,
            library_id: library.id(),
            indexing_enabled: true,
            index_mode: location.index_mode.parse().unwrap_or(IndexMode::Content),
            watch_enabled: true,
        };

        // Start indexing (the indexer will handle incremental updates unless force is true)
        self.start_indexing(library, &managed_location).await
    }
}


impl std::str::FromStr for IndexMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "shallow" => Ok(IndexMode::Shallow),
            "quick" => Ok(IndexMode::Quick),
            "content" => Ok(IndexMode::Content),
            "deep" => Ok(IndexMode::Deep),
            "full" => Ok(IndexMode::Full),
            _ => Err(format!("Unknown index mode: {}", s)),
        }
    }
}