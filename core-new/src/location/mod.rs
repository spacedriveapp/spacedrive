//! Location management - simplified implementation matching core patterns

pub mod manager;

use crate::{
	infrastructure::{
		database::entities::{self, entry::EntryKind},
		events::{Event, EventBus},
		jobs::{handle::JobHandle, output::IndexedOutput, types::JobStatus},
	},
	library::Library,
	operations::indexing::{IndexMode as JobIndexMode, IndexerJob, IndexerJobConfig, PathResolver},
	shared::types::SdPath,
};

use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter, TransactionTrait};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};
use tokio::fs;
use tracing::{error, info, warn};
use uuid::Uuid;

pub use manager::LocationManager;

/// Location creation arguments (simplified from production version)
#[derive(Debug, Serialize, Deserialize)]
pub struct LocationCreateArgs {
	pub path: PathBuf,
	pub name: Option<String>,
	pub index_mode: IndexMode,
}

/// Location indexing mode
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum IndexMode {
	/// Only scan file/directory structure
	Shallow,
	/// Quick scan (metadata only)
	Quick,
	/// Include content hashing for deduplication
	Content,
	/// Full indexing with content analysis and metadata
	Deep,
	/// Full indexing with all features
	Full,
}

impl From<IndexMode> for JobIndexMode {
	fn from(mode: IndexMode) -> Self {
		match mode {
			IndexMode::Shallow => JobIndexMode::Shallow,
			IndexMode::Quick => JobIndexMode::Content,
			IndexMode::Content => JobIndexMode::Content,
			IndexMode::Deep => JobIndexMode::Deep,
			IndexMode::Full => JobIndexMode::Deep,
		}
	}
}

impl From<&str> for IndexMode {
	fn from(s: &str) -> Self {
		match s.to_lowercase().as_str() {
			"shallow" => IndexMode::Shallow,
			"quick" => IndexMode::Quick,
			"content" => IndexMode::Content,
			"deep" => IndexMode::Deep,
			"full" => IndexMode::Full,
			_ => IndexMode::Full,
		}
	}
}

impl std::fmt::Display for IndexMode {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			IndexMode::Shallow => write!(f, "shallow"),
			IndexMode::Quick => write!(f, "quick"),
			IndexMode::Content => write!(f, "content"),
			IndexMode::Deep => write!(f, "deep"),
			IndexMode::Full => write!(f, "full"),
		}
	}
}

/// Managed location representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedLocation {
	pub id: Uuid,
	pub name: String,
	pub path: PathBuf,
	pub device_id: i32,
	pub library_id: Uuid,
	pub indexing_enabled: bool,
	pub index_mode: IndexMode,
	pub watch_enabled: bool,
}

/// Location management errors
#[derive(Debug, thiserror::Error)]
pub enum LocationError {
	#[error("Database error: {0}")]
	Database(#[from] sea_orm::DbErr),
	#[error("Database error: {0}")]
	DatabaseError(String),
	#[error("Path does not exist: {path}")]
	PathNotFound { path: PathBuf },
	#[error("Path not accessible: {path}")]
	PathNotAccessible { path: PathBuf },
	#[error("Location already exists: {path}")]
	LocationExists { path: PathBuf },
	#[error("Location not found: {id}")]
	LocationNotFound { id: Uuid },
	#[error("IO error: {0}")]
	Io(#[from] std::io::Error),
	#[error("Invalid path: {0}")]
	InvalidPath(String),
	#[error("Job error: {0}")]
	Job(#[from] crate::infrastructure::jobs::error::JobError),
	#[error("Other error: {0}")]
	Other(String),
}

pub type LocationResult<T> = Result<T, LocationError>;

/// Create a new location and start indexing (production pattern)
pub async fn create_location(
	library: Arc<Library>,
	events: &EventBus,
	args: LocationCreateArgs,
	device_id: i32,
) -> LocationResult<i32> {
	let path_str = args
		.path
		.to_str()
		.ok_or_else(|| LocationError::InvalidPath("Non-UTF8 path".to_string()))?;

	// Validate path exists
	if !args.path.exists() {
		return Err(LocationError::PathNotFound { path: args.path });
	}

	if !args.path.is_dir() {
		return Err(LocationError::InvalidPath(
			"Path must be a directory".to_string(),
		));
	}

	// Begin transaction to ensure atomicity
	let txn = library.db().conn().begin().await
		.map_err(|e| LocationError::DatabaseError(e.to_string()))?;

	// First, check if an entry already exists for this path
	// We need to create a root entry for the location directory
	let directory_name = args.path
		.file_name()
		.and_then(|n| n.to_str())
		.unwrap_or("Unknown")
		.to_string();

	// Create entry for the location directory
	let entry_model = entities::entry::ActiveModel {
		uuid: Set(Some(Uuid::new_v4())),
		location_id: Set(0), // Will be updated after location is created
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

	let entry_record = entry_model.insert(&txn).await
		.map_err(|e| LocationError::DatabaseError(e.to_string()))?;
	let entry_id = entry_record.id;

	// Add self-reference to closure table
	let self_closure = entities::entry_closure::ActiveModel {
		ancestor_id: Set(entry_id),
		descendant_id: Set(entry_id),
		depth: Set(0),
		..Default::default()
	};
	self_closure.insert(&txn).await
		.map_err(|e| LocationError::DatabaseError(e.to_string()))?;

	// Add to directory_paths table
	let dir_path_entry = entities::directory_paths::ActiveModel {
		entry_id: Set(entry_id),
		path: Set(path_str.to_string()),
		..Default::default()
	};
	dir_path_entry.insert(&txn).await
		.map_err(|e| LocationError::DatabaseError(e.to_string()))?;

	// Check if a location already exists for this entry
	let existing = entities::location::Entity::find()
		.filter(entities::location::Column::EntryId.eq(entry_id))
		.one(&txn)
		.await
		.map_err(|e| LocationError::DatabaseError(e.to_string()))?;

	if existing.is_some() {
		// Rollback transaction
		txn.rollback().await
			.map_err(|e| LocationError::DatabaseError(e.to_string()))?;
		return Err(LocationError::LocationExists { path: args.path });
	}

	// Create location record
	let location_id = Uuid::new_v4();
	let name = args.name.unwrap_or_else(|| {
		args.path
			.file_name()
			.and_then(|n| n.to_str())
			.unwrap_or("Unknown")
			.to_string()
	});

	let location_model = entities::location::ActiveModel {
		id: Set(0), // Auto-increment
		uuid: Set(location_id),
		device_id: Set(device_id),
		entry_id: Set(entry_id),
		name: Set(Some(name.clone())),
		index_mode: Set(args.index_mode.to_string()),
		scan_state: Set("pending".to_string()),
		last_scan_at: Set(None),
		error_message: Set(None),
		total_file_count: Set(0),
		total_byte_size: Set(0),
		created_at: Set(chrono::Utc::now()),
		updated_at: Set(chrono::Utc::now()),
	};

	let location_record = location_model.insert(&txn).await
		.map_err(|e| LocationError::DatabaseError(e.to_string()))?;
	let location_db_id = location_record.id;

	// Update the entry's location_id now that we have it
	let mut entry_active: entities::entry::ActiveModel = entry_record.into();
	entry_active.location_id = Set(location_db_id);
	entry_active.update(&txn).await
		.map_err(|e| LocationError::DatabaseError(e.to_string()))?;

	// Commit transaction
	txn.commit().await
		.map_err(|e| LocationError::DatabaseError(e.to_string()))?;

	info!("Created location '{}' with ID: {}", name, location_db_id);

	// Emit location added event
	events.emit(Event::LocationAdded {
		library_id: library.id(),
		location_id,
		path: args.path.clone(),
	});

	// Start indexing (simplified - in production this goes through proper job manager)
	start_location_indexing(
		library.clone(),
		events,
		location_db_id,
		location_id,
		args.path,
		args.index_mode,
	)
	.await?;

	Ok(location_db_id)
}

/// Start indexing for a location (production implementation)
async fn start_location_indexing(
	library: Arc<Library>,
	events: &EventBus,
	location_db_id: i32,
	location_uuid: Uuid,
	path: PathBuf,
	index_mode: IndexMode,
) -> LocationResult<()> {
	info!("Starting indexing for location: {}", path.display());

	// Update scan state to "running"
	update_location_scan_state(library.clone(), location_db_id, "running", None).await?;

	// Emit indexing started event
	events.emit(Event::IndexingStarted {
		location_id: location_uuid,
	});

	// Get device UUID for SdPath
	let device_uuid = get_device_uuid(library.clone()).await?;
	let location_sd_path = SdPath::new(device_uuid, path.clone());

	// Create and dispatch indexer job through the proper job manager
	let config = IndexerJobConfig::new(location_uuid, location_sd_path, index_mode.into());
	let indexer_job = IndexerJob::new(config);

	match library.jobs().dispatch(indexer_job).await {
		Ok(job_handle) => {
			info!(
				"Successfully dispatched indexer job {} for location: {}",
				job_handle.id(),
				path.display()
			);

			// Monitor job progress asynchronously
			let events_clone = events.clone();
			let library_clone = library.clone();
			let handle_clone = job_handle.clone();

			tokio::spawn(async move {
				monitor_indexing_job(
					handle_clone,
					events_clone,
					library_clone,
					location_db_id,
					location_uuid,
					path,
				)
				.await;
			});
		}
		Err(e) => {
			error!(
				"Failed to dispatch indexer job for {}: {}",
				path.display(),
				e
			);

			// Update scan state to failed
			if let Err(update_err) = update_location_scan_state(
				library.clone(),
				location_db_id,
				"failed",
				Some(e.to_string()),
			)
			.await
			{
				error!("Failed to update scan state: {}", update_err);
			}

			events.emit(Event::IndexingFailed {
				location_id: location_uuid,
				error: e.to_string(),
			});

			return Err(LocationError::Other(format!(
				"Failed to start indexing: {}",
				e
			)));
		}
	}

	Ok(())
}

/// Monitor indexing job progress and update location state accordingly
async fn monitor_indexing_job(
	job_handle: JobHandle,
	events: EventBus,
	library: Arc<Library>,
	location_db_id: i32,
	location_uuid: Uuid,
	path: PathBuf,
) {
	info!(
		"Monitoring indexer job {} for location: {}",
		job_handle.id(),
		path.display()
	);

	// Wait for job completion
	let job_result = job_handle.wait().await;

	match job_result {
		Ok(output) => {
			info!(
				"Indexing completed successfully for location: {}",
				path.display()
			);

			// Parse output to get statistics
			if let Some(indexer_output) = output.as_indexed() {
				// Update location stats
				if let Err(e) = update_location_stats(
					library.clone(),
					location_db_id,
					indexer_output.total_files,
					indexer_output.total_bytes,
				)
				.await
				{
					error!("Failed to update location stats: {}", e);
				}

				// Update scan state to completed
				if let Err(e) =
					update_location_scan_state(library.clone(), location_db_id, "completed", None)
						.await
				{
					error!("Failed to update scan state: {}", e);
				}

				// Emit completion events
				events.emit(Event::IndexingCompleted {
					location_id: location_uuid,
					total_files: indexer_output.total_files,
					total_dirs: indexer_output.total_dirs,
				});

				events.emit(Event::FilesIndexed {
					library_id: library.id(),
					location_id: location_uuid,
					count: indexer_output.total_files as usize,
				});

				info!(
					"Location indexing completed: {} ({} files, {} dirs, {} bytes)",
					path.display(),
					indexer_output.total_files,
					indexer_output.total_dirs,
					indexer_output.total_bytes
				);
			} else {
				warn!("Job completed but output format was unexpected");

				// Update scan state to completed anyway
				if let Err(e) =
					update_location_scan_state(library.clone(), location_db_id, "completed", None)
						.await
				{
					error!("Failed to update scan state: {}", e);
				}
			}
		}
		Err(e) => {
			error!("Indexing failed for {}: {}", path.display(), e);

			// Update scan state to failed
			if let Err(update_err) = update_location_scan_state(
				library.clone(),
				location_db_id,
				"failed",
				Some(e.to_string()),
			)
			.await
			{
				error!("Failed to update scan state: {}", update_err);
			}

			events.emit(Event::IndexingFailed {
				location_id: location_uuid,
				error: e.to_string(),
			});
		}
	}
}

/// Scan directory to get basic stats
async fn scan_directory_stats(path: &PathBuf) -> Result<(u64, u64), std::io::Error> {
	let mut file_count = 0u64;
	let mut total_size = 0u64;

	let mut stack = vec![path.clone()];

	while let Some(current_path) = stack.pop() {
		if let Ok(mut entries) = fs::read_dir(&current_path).await {
			while let Ok(Some(entry)) = entries.next_entry().await {
				if let Ok(metadata) = entry.metadata().await {
					if metadata.is_file() {
						file_count += 1;
						total_size += metadata.len();
					} else if metadata.is_dir() {
						stack.push(entry.path());
					}
				}
			}
		}
	}

	Ok((file_count, total_size))
}

/// Update location scan state
async fn update_location_scan_state(
	library: Arc<Library>,
	location_id: i32,
	state: &str,
	error_message: Option<String>,
) -> LocationResult<()> {
	let location = entities::location::Entity::find_by_id(location_id)
		.one(library.db().conn())
		.await?
		.ok_or_else(|| LocationError::LocationNotFound { id: Uuid::nil() })?;

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
	library: Arc<Library>,
	location_id: i32,
	file_count: u64,
	total_size: u64,
) -> LocationResult<()> {
	let location = entities::location::Entity::find_by_id(location_id)
		.one(library.db().conn())
		.await?
		.ok_or_else(|| LocationError::LocationNotFound { id: Uuid::nil() })?;

	let mut active_location: entities::location::ActiveModel = location.into();
	active_location.total_file_count = Set(file_count as i64);
	active_location.total_byte_size = Set(total_size as i64);
	active_location.updated_at = Set(chrono::Utc::now());

	active_location.update(library.db().conn()).await?;
	Ok(())
}

/// Get device UUID for current device
async fn get_device_uuid(library: Arc<Library>) -> LocationResult<Uuid> {
	let device = entities::device::Entity::find()
		.one(library.db().conn())
		.await?
		.ok_or_else(|| LocationError::InvalidPath("No device found".to_string()))?;

	Ok(device.uuid)
}

/// List all locations for a library
pub async fn list_locations(
	library: Arc<Library>,
) -> LocationResult<Vec<entities::location::Model>> {
	Ok(entities::location::Entity::find()
		.all(library.db().conn())
		.await?)
}
