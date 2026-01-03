//! Location management - simplified implementation matching core patterns

pub mod manager;

use crate::{
	domain::addressing::SdPath,
	infra::{
		db::entities::{self, entry::EntryKind},
		event::{Event, EventBus},
		job::{handle::JobHandle, output::IndexedOutput, types::JobStatus},
	},
	library::Library,
	ops::indexing::{
		rules::RuleToggles, IndexMode as JobIndexMode, IndexerJob, IndexerJobConfig, PathResolver,
	},
};

use sea_orm::{
	ActiveModelTrait,
	ActiveValue::{NotSet, Set},
	ColumnTrait, EntityTrait, QueryFilter, TransactionTrait,
};
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
	/// Location exists but is not indexed
	None,
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
			IndexMode::None => JobIndexMode::None,
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
			"none" => IndexMode::None,
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
			IndexMode::None => write!(f, "none"),
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
	Job(#[from] crate::infra::job::error::JobError),
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
	let txn = library
		.db()
		.conn()
		.begin()
		.await
		.map_err(|e| LocationError::DatabaseError(e.to_string()))?;

	// First, check if an entry already exists for this path
	// We need to create a root entry for the location directory
	let directory_name = args
		.path
		.file_name()
		.and_then(|n| n.to_str())
		.unwrap_or("Unknown")
		.to_string();

	// Create entry for the location directory
	let now = chrono::Utc::now();
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
		created_at: Set(now),
		modified_at: Set(now),
		accessed_at: Set(None),
		indexed_at: Set(Some(now)), // CRITICAL: Must be set for sync to work (enables StateChange emission)
		permissions: Set(None),
		inode: Set(None),
		parent_id: Set(None),            // Location root has no parent
		device_id: Set(Some(device_id)), // CRITICAL: Must be set for device-owned sync queries
		..Default::default()
	};

	let entry_record = entry_model
		.insert(&txn)
		.await
		.map_err(|e| LocationError::DatabaseError(e.to_string()))?;
	let entry_id = entry_record.id;

	// Add self-reference to closure table
	let self_closure = entities::entry_closure::ActiveModel {
		ancestor_id: Set(entry_id),
		descendant_id: Set(entry_id),
		depth: Set(0),
		..Default::default()
	};
	self_closure
		.insert(&txn)
		.await
		.map_err(|e| LocationError::DatabaseError(e.to_string()))?;

	// Add to directory_paths table
	let dir_path_entry = entities::directory_paths::ActiveModel {
		entry_id: Set(entry_id),
		path: Set(path_str.to_string()),
		..Default::default()
	};
	dir_path_entry
		.insert(&txn)
		.await
		.map_err(|e| LocationError::DatabaseError(e.to_string()))?;

	// Check if a location already exists for this entry
	let existing = entities::location::Entity::find()
		.filter(entities::location::Column::EntryId.eq(entry_id))
		.one(&txn)
		.await
		.map_err(|e| LocationError::DatabaseError(e.to_string()))?;

	if existing.is_some() {
		// Rollback transaction
		txn.rollback()
			.await
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
		id: NotSet, // Auto-increment handled by database
		uuid: Set(location_id),
		device_id: Set(device_id),
		entry_id: Set(Some(entry_id)),
		name: Set(Some(name.clone())),
		index_mode: Set(args.index_mode.to_string()),
		scan_state: Set("pending".to_string()),
		last_scan_at: Set(None),
		error_message: Set(None),
		total_file_count: Set(0),
		total_byte_size: Set(0),
		job_policies: Set(None), // Use defaults
		created_at: Set(chrono::Utc::now()),
		updated_at: Set(chrono::Utc::now()),
	};

	let location_record = location_model
		.insert(&txn)
		.await
		.map_err(|e| LocationError::DatabaseError(e.to_string()))?;
	let location_db_id = location_record.id;

	// Commit transaction
	txn.commit()
		.await
		.map_err(|e| LocationError::DatabaseError(e.to_string()))?;

	info!("Created location '{}' with ID: {}", name, location_db_id);

	// Emit StateChange event for root entry
	// The raw transaction above doesn't use TransactionManager, so we must manually emit
	// This ensures the root directory syncs to other devices BEFORE its children

	// Get device UUID from device_id (internal ID)
	let device_record = entities::device::Entity::find_by_id(device_id)
		.one(library.db().conn())
		.await
		.map_err(|e| LocationError::DatabaseError(e.to_string()))?
		.ok_or_else(|| LocationError::DatabaseError("Device not found".to_string()))?;

	let root_entry_uuid = entry_record.uuid.expect("Root entry should have UUID");
	let root_entry_data = serde_json::to_value(&entry_record).map_err(|e| {
		LocationError::DatabaseError(format!("Failed to serialize root entry: {}", e))
	})?;

	library
		.transaction_manager()
		.commit_device_owned(
			library.id(),
			"entry",
			root_entry_uuid,
			device_record.uuid,
			root_entry_data,
		)
		.await
		.map_err(|e| {
			LocationError::DatabaseError(format!(
				"Failed to emit StateChange for root entry: {}",
				e
			))
		})?;

	// Emit StateChange event for location
	// This ensures the location syncs to other devices with proper entry_id
	let location_data = serde_json::to_value(&location_record).map_err(|e| {
		LocationError::DatabaseError(format!("Failed to serialize location: {}", e))
	})?;

	library
		.transaction_manager()
		.commit_device_owned(
			library.id(),
			"location",
			location_id,
			device_record.uuid,
			location_data,
		)
		.await
		.map_err(|e| {
			LocationError::DatabaseError(format!("Failed to emit StateChange for location: {}", e))
		})?;

	// Emit location added event (for UI)
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

	// Get device slug for SdPath
	let device_slug = get_device_slug(library.clone()).await?;
	let location_sd_path = SdPath::new(device_slug, path.clone());

	// Create and dispatch indexer job through the proper job manager
	let lib_cfg = library.config().await;
	let idx_cfg = lib_cfg.settings.indexer;
	let mut config = IndexerJobConfig::new(location_uuid, location_sd_path, index_mode.into());
	config.rule_toggles = RuleToggles {
		no_system_files: idx_cfg.no_system_files,
		no_hidden: idx_cfg.no_hidden,
		no_git: idx_cfg.no_git,
		gitignore: idx_cfg.gitignore,
		only_images: idx_cfg.only_images,
		no_dev_dirs: idx_cfg.no_dev_dirs,
	};
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

/// Get device slug for current device
async fn get_device_slug(_library: Arc<Library>) -> LocationResult<String> {
	// Get the current device slug from the global state
	let device_slug = crate::device::get_current_device_slug();

	if device_slug.is_empty() {
		return Err(LocationError::InvalidPath(
			"Current device slug not initialized".to_string(),
		));
	}

	Ok(device_slug)
}

/// List all locations for a library
pub async fn list_locations(
	library: Arc<Library>,
) -> LocationResult<Vec<entities::location::Model>> {
	Ok(entities::location::Entity::find()
		.all(library.db().conn())
		.await?)
}
