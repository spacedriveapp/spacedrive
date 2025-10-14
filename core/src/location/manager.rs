//! Location Manager - Orchestrates location lifecycle and indexing

use super::{IndexMode, LocationError, LocationResult, ManagedLocation};
use crate::{
	domain::addressing::SdPath,
	infra::{
		db::entities::{self, entry::EntryKind},
		event::{Event, EventBus},
		job::{manager::JobManager, traits::Job},
	},
	library::Library,
	ops::indexing::{
		job::{IndexerJob, IndexerJobConfig},
		PathResolver,
	},
};
use sea_orm::{
	ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter,
	TransactionTrait,
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
		sd_path: crate::domain::addressing::SdPath,
		name: Option<String>,
		device_id: i32,
		index_mode: IndexMode,
		action_context: Option<crate::infra::action::context::ActionContext>,
	) -> LocationResult<(Uuid, String)> {
		info!("Adding location: {}", sd_path);

		// Validate the path based on type
		match &sd_path {
			crate::domain::addressing::SdPath::Physical { path, .. } => {
				self.validate_physical_path(path).await?;
			}
			crate::domain::addressing::SdPath::Cloud { volume_fingerprint, .. } => {
				self.validate_cloud_path(&library, volume_fingerprint).await?;
			}
			crate::domain::addressing::SdPath::Content { .. } => {
				return Err(LocationError::InvalidPath(
					"Content paths cannot be used as locations".to_string(),
				));
			}
		}

		// Begin transaction
		let txn = library.db().conn().begin().await?;

		// Get directory name and path string from SdPath
		let (directory_name, path_str) = match &sd_path {
			crate::domain::addressing::SdPath::Physical { path, .. } => {
				let name = path
					.file_name()
					.and_then(|n| n.to_str())
					.unwrap_or("Unknown")
					.to_string();
				let path_str = path.to_string_lossy().to_string();
				(name, path_str)
			}
			crate::domain::addressing::SdPath::Cloud { volume_fingerprint, path } => {
				let name = path
					.split('/')
					.last()
					.filter(|s| !s.is_empty())
					.unwrap_or("Cloud Root")
					.to_string();
				let path_str = format!("cloud://{}/{}", volume_fingerprint.0, path);
				(name, path_str)
			}
			_ => unreachable!("Content paths already rejected"),
		};

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
			path: Set(path_str.clone()),
			..Default::default()
		};
		dir_path_entry.insert(&txn).await?;

		// Create the location record
		let location_id = Uuid::new_v4();
		let display_name = name.unwrap_or_else(|| directory_name.clone());

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

		// Sync location to other devices (has FK relationships: device_id, entry_id)
		use crate::infra::sync::ChangeType;
		library
			.sync_model_with_db(&location_record, ChangeType::Insert, library.db().conn())
			.await
			.map_err(|e| {
				warn!("Failed to sync location: {}", e);
				// Don't fail the operation if sync fails - location was created successfully
				e
			})
			.ok(); // Convert to Option and discard (we already logged the error)

		// Create managed location with path
		// For cloud locations, use the actual cloud path string for proper watcher filtering
		let location_path = match &sd_path {
			crate::domain::addressing::SdPath::Physical { path, .. } => path.clone(),
			crate::domain::addressing::SdPath::Cloud { .. } => PathBuf::from(&path_str),
			_ => unreachable!(),
		};

		let managed_location = ManagedLocation {
			id: location_id,
			name: display_name.clone(),
			path: location_path.clone(),
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
			path: location_path,
		});

		// Also emit indexing started event
		self.events.emit(Event::IndexingStarted { location_id });

		// Start indexing job with action context, passing the SdPath directly
		let job_id = match self
			.start_indexing_with_context_and_path(library, &managed_location, sd_path.clone(), action_context)
			.await
		{
			Ok(job_id) => {
				info!(
					"Started indexing job {} for location '{}'",
					job_id, display_name
				);

				// Emit job started event
				self.events.emit(Event::JobStarted {
					job_id: job_id.clone(),
					job_type: "Indexing".to_string(),
				});

				job_id
			}
			Err(e) => {
				error!(
					"Failed to start indexing for location '{}': {}",
					display_name, e
				);
				// Return empty job ID if indexing fails
				String::new()
			}
		};

		info!("Successfully added location '{}'", display_name);
		Ok((location_id, job_id))
	}

	/// Start indexing for a location
	pub async fn start_indexing(
		&self,
		library: Arc<Library>,
		location: &ManagedLocation,
	) -> LocationResult<String> {
		self.start_indexing_with_context(library, location, None)
			.await
	}

	/// Start indexing for a location with action context
	pub async fn start_indexing_with_context(
		&self,
		library: Arc<Library>,
		location: &ManagedLocation,
		action_context: Option<crate::infra::action::context::ActionContext>,
	) -> LocationResult<String> {
		// Construct SdPath from location
		let device_uuid = self.get_device_uuid(&library, location.device_id).await?;
		let location_sd_path = SdPath::new(device_uuid, location.path.clone());

		self.start_indexing_with_context_and_path(
			library,
			location,
			location_sd_path,
			action_context,
		)
		.await
	}

	/// Start indexing for a location with action context and explicit SdPath
	pub async fn start_indexing_with_context_and_path(
		&self,
		library: Arc<Library>,
		location: &ManagedLocation,
		location_sd_path: SdPath,
		action_context: Option<crate::infra::action::context::ActionContext>,
	) -> LocationResult<String> {
		info!(
			"Starting indexing for location '{}' at {} in mode {:?}",
			location.name, location_sd_path, location.index_mode
		);

		// Update scan state to "scanning"
		self.update_scan_state(&library, location.id, "scanning", None)
			.await?;

		// Create indexer job using new configuration pattern
		let config =
			IndexerJobConfig::new(location.id, location_sd_path.clone(), location.index_mode.into());
		let indexer_job = IndexerJob::new(config);

		// Submit to job manager with action context
		let job_manager = library.jobs();
		let job_handle = job_manager
			.dispatch_with_priority(
				indexer_job,
				crate::infra::job::types::JobPriority::NORMAL,
				action_context,
			)
			.await?;
		let job_id = job_handle.id();

		info!(
			"Started indexing job {} for location '{}' at {}",
			job_id, location.name, location_sd_path
		);

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

	/// Validate a physical filesystem path before creating a location
	async fn validate_physical_path(&self, path: &PathBuf) -> LocationResult<()> {
		// Check if path exists
		if !path.exists() {
			return Err(LocationError::PathNotFound { path: path.clone() });
		}

		// Check if it's a directory
		let metadata = fs::metadata(path).await?;
		if !metadata.is_dir() {
			return Err(LocationError::InvalidPath(
				"Path must be a directory".to_string(),
			));
		}

		// Check if we have read permissions
		match fs::read_dir(path).await {
			Ok(_) => Ok(()),
			Err(e) => match e.kind() {
				std::io::ErrorKind::PermissionDenied => {
					Err(LocationError::PathNotAccessible { path: path.clone() })
				}
				_ => Err(LocationError::Io(e)),
			},
		}
	}

	/// Validate a cloud volume before creating a location
	async fn validate_cloud_path(&self, library: &Library, volume_fingerprint: &crate::volume::VolumeFingerprint) -> LocationResult<()> {
		// Check if volume exists in database
		let db = library.db().conn();
		let volume = entities::volume::Entity::find()
			.filter(entities::volume::Column::Fingerprint.eq(volume_fingerprint.0.clone()))
			.one(db)
			.await
			.map_err(|e| LocationError::Other(format!("Database error: {}", e)))?
			.ok_or_else(|| {
				LocationError::Other(format!("Cloud volume {} not found", volume_fingerprint.0))
			})?;

		// TODO: Validate that we can connect to the volume
		// This would require accessing the VolumeManager and VolumeBackend

		Ok(())
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
	pub async fn list_locations(&self, library: &Library) -> LocationResult<Vec<ManagedLocation>> {
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
