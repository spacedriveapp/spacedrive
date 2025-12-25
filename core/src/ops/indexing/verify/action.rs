//! Index integrity verification action

use super::{input::IndexVerifyInput, output::*};
use crate::{
	context::CoreContext,
	domain::addressing::SdPath,
	infra::{
		action::{error::ActionError, LibraryAction},
		db::entities,
	},
	ops::indexing::{
		database_storage::DatabaseStorage,
		ephemeral::EphemeralIndex,
		job::{IndexMode, IndexPersistence, IndexScope, IndexerJob, IndexerJobConfig},
		path_resolver::PathResolver,
		state::EntryKind,
	},
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use std::{
	collections::{HashMap, HashSet},
	path::{Path, PathBuf},
	sync::Arc,
	time::Instant,
};
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct IndexVerifyAction {
	input: IndexVerifyInput,
}

impl LibraryAction for IndexVerifyAction {
	type Input = IndexVerifyInput;
	type Output = IndexVerifyOutput;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		// Validate input
		input
			.validate()
			.map_err(|errors| format!("Validation failed: {}", errors.join("; ")))?;

		Ok(Self { input })
	}

	async fn execute(
		self,
		library: Arc<crate::library::Library>,
		context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let start = Instant::now();
		let path = self.input.path.clone();

		tracing::info!(
			"Starting index integrity verification for: {}",
			path.display()
		);

		// Step 1: Scan filesystem to get current state
		let fs_entries = self.run_ephemeral_index(&library, &path).await?;

		// Step 2: Query database for existing entries in this path
		let db_entries = self.query_database_entries(&library, &path).await?;

		// Step 3: Compare and generate report
		let mut report = self.compare_indexes(fs_entries, db_entries, &path).await?;

		// Generate summary
		report.generate_summary();

		let duration = start.elapsed();

		tracing::info!(
			"Index verification complete in {:.2}s: {}",
			duration.as_secs_f64(),
			report.summary
		);

		Ok(IndexVerifyOutput {
			is_valid: report.is_valid(),
			report,
			path,
			duration_secs: duration.as_secs_f64(),
		})
	}

	fn action_kind(&self) -> &'static str {
		"indexing.verify"
	}
}

impl IndexVerifyAction {
	/// Run ephemeral indexing to get current filesystem state using the real IndexerJob
	async fn run_ephemeral_index(
		&self,
		library: &Arc<crate::library::Library>,
		path: &Path,
	) -> Result<HashMap<PathBuf, crate::ops::indexing::database_storage::EntryMetadata>, ActionError>
	{
		use tokio::sync::RwLock;

		tracing::debug!("Running ephemeral indexer job on {}", path.display());

		// Create ephemeral index storage that we'll share with the job
		let ephemeral_index =
			Arc::new(RwLock::new(EphemeralIndex::new().map_err(|e| {
				ActionError::from(std::io::Error::new(std::io::ErrorKind::Other, e))
			})?));

		// Create indexer job config for ephemeral scanning
		let config = IndexerJobConfig {
			location_id: None, // Ephemeral - no location
			path: SdPath::local(path),
			mode: IndexMode::Deep, // Full metadata extraction including inodes
			scope: IndexScope::Recursive,
			persistence: IndexPersistence::Ephemeral,
			max_depth: None,
			rule_toggles: Default::default(),
			run_in_background: false,
		};

		// Create the job and set our ephemeral index storage BEFORE dispatching
		let mut job = IndexerJob::new(config);
		job.set_ephemeral_index(ephemeral_index.clone());

		// Dispatch the job
		let job_handle =
			library.jobs().dispatch(job).await.map_err(|e| {
				ActionError::Internal(format!("Failed to dispatch indexer job: {}", e))
			})?;

		let job_id = job_handle.id();
		tracing::debug!(
			"Waiting for ephemeral indexer job {} to complete...",
			job_id
		);

		// Wait for the job to complete using the handle's built-in wait mechanism
		job_handle
			.wait()
			.await
			.map_err(|e| ActionError::Internal(format!("Ephemeral indexer job failed: {}", e)))?;

		tracing::debug!(
			"Ephemeral indexer job {} completed, extracting results",
			job_id
		);

		// Extract the results from our shared ephemeral index
		let entries = {
			let index = ephemeral_index.read().await;
			index.entries()
		};

		tracing::debug!(
			"Collected {} filesystem entries from ephemeral index",
			entries.len()
		);

		Ok(entries)
	}

	/// Query database for all entries under the given path
	async fn query_database_entries(
		&self,
		library: &Arc<crate::library::Library>,
		root_path: &Path,
	) -> Result<HashMap<PathBuf, (entities::entry::Model, PathBuf)>, ActionError> {
		tracing::debug!("Querying database entries for {}", root_path.display());

		let db = library.db().conn();
		let root_path_str = root_path.to_string_lossy().to_string();

		// First, find which location this path belongs to
		let locations = entities::location::Entity::find()
			.all(db)
			.await
			.map_err(|e| ActionError::Internal(format!("Failed to query locations: {}", e)))?;

		let mut target_location = None;
		for loc in locations {
			let entry_id = loc
				.entry_id
				.ok_or_else(|| ActionError::Internal("Location has no entry_id".to_string()))?;
			let loc_path = PathResolver::get_full_path(db, entry_id)
				.await
				.map_err(|e| {
					ActionError::Internal(format!("Failed to get location path: {}", e))
				})?;

			// Check if our target path is within this location
			if root_path.starts_with(&loc_path) || root_path == loc_path {
				target_location = Some((loc, loc_path));
				break;
			}
		}

		let Some((location, location_path)) = target_location else {
			return Err(ActionError::Internal(format!(
				"Path {} does not belong to any managed location",
				root_path.display()
			)));
		};

		tracing::debug!(
			"Found location {} for path {}",
			location.name.as_deref().unwrap_or("Unknown"),
			root_path.display()
		);

		let mut entries_map = HashMap::new();

		// Find the directory entry for this specific path
		let root_entry = entities::directory_paths::Entity::find()
			.filter(entities::directory_paths::Column::Path.eq(&root_path_str))
			.one(db)
			.await
			.map_err(|e| {
				ActionError::Internal(format!("Failed to query directory paths: {}", e))
			})?;

		if let Some(root_dir) = root_entry {
			// Get all descendant entries using closure table
			let descendant_closures = entities::entry_closure::Entity::find()
				.filter(entities::entry_closure::Column::AncestorId.eq(root_dir.entry_id))
				.all(db)
				.await
				.map_err(|e| {
					ActionError::Internal(format!("Failed to query entry closure: {}", e))
				})?;

			let descendant_ids: Vec<i32> = descendant_closures
				.iter()
				.map(|ec| ec.descendant_id)
				.collect();

			if descendant_ids.is_empty() {
				tracing::warn!("No descendants found for root directory");
				return Ok(entries_map);
			}

			// Fetch all entries
			let entries = entities::entry::Entity::find()
				.filter(entities::entry::Column::Id.is_in(descendant_ids))
				.all(db)
				.await
				.map_err(|e| ActionError::Internal(format!("Failed to query entries: {}", e)))?;

			tracing::debug!("Found {} descendant entries", entries.len());

			// Resolve full paths for all entries
			for entry in entries {
				let full_path = PathResolver::get_full_path(db, entry.id)
					.await
					.unwrap_or_else(|_| PathBuf::from(&entry.name));

				entries_map.insert(full_path.clone(), (entry, full_path));
			}
		} else {
			// Path is within a location but not the root - need to find the entry ID for this path
			// by traversing from the location root
			tracing::debug!("Path is subdirectory of location, traversing from root");

			let relative_path = root_path.strip_prefix(&location_path).map_err(|e| {
				ActionError::Internal(format!("Failed to compute relative path: {}", e))
			})?;

			// Get path components
			let components: Vec<&str> = relative_path
				.components()
				.filter_map(|c| c.as_os_str().to_str())
				.collect();

			if components.is_empty() {
				// This is the location root, use location.entry_id
				let root_entry_id = location.entry_id;

				// Get all descendants using closure table
				let descendant_closures = entities::entry_closure::Entity::find()
					.filter(entities::entry_closure::Column::AncestorId.eq(root_entry_id))
					.all(db)
					.await
					.map_err(|e| {
						ActionError::Internal(format!("Failed to query entry closure: {}", e))
					})?;

				let descendant_ids: Vec<i32> = descendant_closures
					.iter()
					.map(|ec| ec.descendant_id)
					.collect();

				let entries = entities::entry::Entity::find()
					.filter(entities::entry::Column::Id.is_in(descendant_ids))
					.all(db)
					.await
					.map_err(|e| {
						ActionError::Internal(format!("Failed to query entries: {}", e))
					})?;

				for entry in entries {
					let full_path = PathResolver::get_full_path(db, entry.id)
						.await
						.unwrap_or_else(|_| PathBuf::from(&entry.name));
					entries_map.insert(full_path.clone(), (entry, full_path));
				}
			} else {
				// Traverse from location root to find the target directory
				let mut current_parent_id = location
					.entry_id
					.ok_or_else(|| ActionError::Internal("Location has no entry_id".to_string()))?;

				for component in &components {
					// Find child with this name
					let child = entities::entry::Entity::find()
						.filter(entities::entry::Column::ParentId.eq(current_parent_id))
						.filter(entities::entry::Column::Name.eq(*component))
						.one(db)
						.await
						.map_err(|e| {
							ActionError::Internal(format!("Failed to query entry: {}", e))
						})?;

					if let Some(c) = child {
						current_parent_id = c.id;
					} else {
						// Path not found in database
						return Ok(entries_map);
					}
				}

				// Get all descendants of this subdirectory
				let descendant_closures = entities::entry_closure::Entity::find()
					.filter(entities::entry_closure::Column::AncestorId.eq(current_parent_id))
					.all(db)
					.await
					.map_err(|e| {
						ActionError::Internal(format!("Failed to query entry closure: {}", e))
					})?;

				let descendant_ids: Vec<i32> = descendant_closures
					.iter()
					.map(|ec| ec.descendant_id)
					.collect();

				let entries = entities::entry::Entity::find()
					.filter(entities::entry::Column::Id.is_in(descendant_ids))
					.all(db)
					.await
					.map_err(|e| {
						ActionError::Internal(format!("Failed to query entries: {}", e))
					})?;

				for entry in entries {
					let full_path = PathResolver::get_full_path(db, entry.id)
						.await
						.unwrap_or_else(|_| PathBuf::from(&entry.name));
					entries_map.insert(full_path.clone(), (entry, full_path));
				}
			}

			tracing::debug!("Found {} entries in database", entries_map.len());
		}

		Ok(entries_map)
	}

	/// Compare ephemeral index with database entries
	async fn compare_indexes(
		&self,
		fs_entries: HashMap<PathBuf, crate::ops::indexing::database_storage::EntryMetadata>,
		mut db_entries: HashMap<PathBuf, (entities::entry::Model, PathBuf)>,
		root_path: &Path,
	) -> Result<IntegrityReport, ActionError> {
		tracing::debug!("Comparing filesystem and database indexes");

		let mut report = IntegrityReport::new();

		tracing::debug!(
			"Comparing {} filesystem entries with {} database entries",
			fs_entries.len(),
			db_entries.len()
		);

		// Remove the root path itself from db_entries - the ephemeral indexer doesn't
		// create an entry for the root directory it's scanning, only its contents
		db_entries.remove(root_path);

		// Count files and directories
		for (_path, metadata) in &fs_entries {
			match metadata.kind {
				EntryKind::File => report.filesystem_file_count += 1,
				EntryKind::Directory => report.filesystem_dir_count += 1,
				_ => {}
			}
		}

		for (_path, (entry, _)) in &db_entries {
			let kind = entry.entry_kind();
			match kind {
				entities::entry::EntryKind::File => report.database_file_count += 1,
				entities::entry::EntryKind::Directory => report.database_dir_count += 1,
				_ => {}
			}
		}

		// Build sets for comparison
		// On case-insensitive filesystems (macOS), normalize paths to lowercase for comparison
		#[cfg(target_os = "macos")]
		let normalize_path = |pb: &PathBuf| -> String { pb.to_string_lossy().to_lowercase() };

		#[cfg(not(target_os = "macos"))]
		let normalize_path = |pb: &PathBuf| -> String { pb.to_string_lossy().to_string() };

		// Create normalized path maps for case-insensitive comparison on macOS
		let fs_normalized: HashMap<String, PathBuf> = fs_entries
			.keys()
			.map(|p| (normalize_path(p), p.clone()))
			.collect();

		let db_normalized: HashMap<String, PathBuf> = db_entries
			.keys()
			.map(|p| (normalize_path(p), p.clone()))
			.collect();

		let fs_paths: HashSet<String> = fs_normalized.keys().cloned().collect();
		let db_paths: HashSet<String> = db_normalized.keys().cloned().collect();

		// Find missing from index (in filesystem but not in DB)
		for norm_path in fs_paths.difference(&db_paths) {
			let path = &fs_normalized[norm_path];
			report
				.missing_from_index
				.push(IntegrityDifference::missing_from_index(path.clone()));
		}

		// Find stale in index (in DB but not on filesystem)
		for norm_path in db_paths.difference(&fs_paths) {
			let path = &db_normalized[norm_path];
			report
				.stale_in_index
				.push(IntegrityDifference::stale_in_index(path.clone()));
		}

		// Find metadata mismatches (in both but with different data)
		for norm_path in fs_paths.intersection(&db_paths) {
			let fs_path = &fs_normalized[norm_path];
			let db_path = &db_normalized[norm_path];

			if let (Some(fs_meta), Some((db_entry, _))) =
				(fs_entries.get(fs_path), db_entries.get(db_path))
			{
				// Check size
				let fs_size = fs_meta.size;
				let db_size = db_entry.size as u64;
				if fs_size != db_size {
					report
						.metadata_mismatches
						.push(IntegrityDifference::size_mismatch_with_debug(
							fs_path.clone(),
							fs_size,
							db_size,
							db_entry.id,
							db_entry.name.clone(),
						));
				}

				// Check modified time (allow 1 second tolerance for filesystem precision)
				if let Some(fs_modified) = fs_meta.modified {
					if let Ok(fs_duration) = fs_modified.duration_since(std::time::UNIX_EPOCH) {
						let fs_secs = fs_duration.as_secs() as i64;
						let db_secs = db_entry.modified_at.timestamp();

						if (fs_secs - db_secs).abs() > 1 {
							report.metadata_mismatches.push(
								IntegrityDifference::modified_time_mismatch(
									fs_path.clone(),
									format!("{}", fs_secs),
									format!("{}", db_secs),
								),
							);
						}
					}
				}

				// Check inode if available
				if let (Some(fs_inode), Some(db_inode)) = (fs_meta.inode, db_entry.inode) {
					if fs_inode != db_inode as u64 {
						report.metadata_mismatches.push(IntegrityDifference {
							path: fs_path.clone(),
							issue_type: IssueType::InodeMismatch,
							expected: Some(format!("{}", fs_inode)),
							actual: Some(format!("{}", db_inode)),
							description: format!("Inode mismatch for {}", fs_path.display()),
							db_entry_id: Some(db_entry.id),
							db_entry_name: Some(db_entry.name.clone()),
						});
					}
				}
			}
		}

		tracing::debug!(
			"Comparison complete: {} missing, {} stale, {} metadata mismatches",
			report.missing_from_index.len(),
			report.stale_in_index.len(),
			report.metadata_mismatches.len()
		);

		Ok(report)
	}
}

crate::register_library_action!(IndexVerifyAction, "indexing.verify");
