//! Simplified FileCopyJob using the Strategy Pattern

use super::{database::CopyDatabaseQuery, input::CopyMethod, routing::CopyStrategyRouter};
use crate::{
	infrastructure::jobs::generic_progress::{GenericProgress, ToGenericProgress},
	infrastructure::jobs::{prelude::*, traits::Resourceful},
	shared::types::{SdPath, SdPathBatch},
};
use serde::{Deserialize, Serialize};
use std::{
	collections::HashMap,
	path::PathBuf,
	sync::{Arc, Mutex},
	time::{Duration, Instant},
};
use uuid::Uuid;

/// Move operation modes for UI context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MoveMode {
	/// Standard move operation
	Move,
	/// Rename a single file/directory
	Rename,
	/// Cut and paste operation (same as move but different UX context)
	Cut,
}

/// Options for file copy operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyOptions {
	pub overwrite: bool,
	pub verify_checksum: bool,
	pub preserve_timestamps: bool,
	pub delete_after_copy: bool,
	pub move_mode: Option<MoveMode>,
	pub copy_method: CopyMethod,
}

impl Default for CopyOptions {
	fn default() -> Self {
		Self {
			overwrite: false,
			verify_checksum: false,
			preserve_timestamps: true,
			delete_after_copy: false,
			move_mode: None,
			copy_method: CopyMethod::Auto,
		}
	}
}

/// File copy job using the Strategy Pattern
#[derive(Debug, Serialize, Deserialize, Job)]
pub struct FileCopyJob {
	pub sources: SdPathBatch,
	pub destination: SdPath,
	#[serde(default)]
	pub options: CopyOptions,

	// Internal state for resumption
	#[serde(default)]
	pub completed_indices: Vec<usize>,
	#[serde(skip, default = "Instant::now")]
	started_at: Instant,
}

impl Job for FileCopyJob {
	const NAME: &'static str = "file_copy";
	const RESUMABLE: bool = true;
	const DESCRIPTION: Option<&'static str> = Some("Copy or move files to a destination");
}

impl crate::infrastructure::jobs::traits::DynJob for FileCopyJob {
	fn as_any(&self) -> &dyn std::any::Any {
		self
	}

	fn job_name(&self) -> &'static str {
		Self::NAME
	}
}

#[async_trait::async_trait]
impl JobHandler for FileCopyJob {
	type Output = FileCopyOutput;

	async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
		ctx.log(format!(
			"Starting copy operation on {} files",
			self.sources.paths.len()
		));

		// Phase 1: Initializing
		let progress = CopyProgress {
			phase: CopyPhase::Initializing,
			current_file: String::new(),
			files_copied: 0,
			total_files: 0,
			bytes_copied: 0,
			total_bytes: 0,
			current_operation: "Initializing copy operation".to_string(),
			estimated_remaining: None,
			preparation_complete: false,
			error_count: 0,
		};
		ctx.progress(Progress::generic(progress.to_generic_progress()));

		// Group by device for efficient processing
		let by_device: HashMap<Uuid, Vec<SdPath>> = self
			.sources
			.by_device()
			.into_iter()
			.map(|(device_id, paths)| (device_id, paths.into_iter().cloned().collect()))
			.collect();

		let mut copied_count = 0;
		let mut total_bytes = 0u64;
		let mut failed_copies = Vec::new();
		let is_move = self.options.delete_after_copy;
		let volume_manager = ctx.volume_manager();

		// Phase 2: Database Query - Try to get instant estimates
		let progress = CopyProgress {
			phase: CopyPhase::DatabaseQuery,
			current_file: String::new(),
			files_copied: 0,
			total_files: 0,
			bytes_copied: 0,
			total_bytes: 0,
			current_operation: "Querying database for file information...".to_string(),
			estimated_remaining: None,
			preparation_complete: false,
			error_count: 0,
		};
		ctx.progress(Progress::generic(progress.to_generic_progress()));

		// Try to get estimates from database
		let db_estimates = {
			let db_query = CopyDatabaseQuery::new(ctx.library_db().clone());
			match db_query.get_estimates_for_paths(&self.sources.paths).await {
				Ok(estimates) => {
					ctx.log(format!(
						"Database estimates: {} files, {} bytes ({:.0}% coverage)",
						estimates.file_count,
						estimates.total_size,
						estimates.confidence() * 100.0
					));
					Some(estimates)
				}
				Err(e) => {
					ctx.log(format!(
						"Database query failed, will calculate from filesystem: {}",
						e
					));
					None
				}
			}
		};

		// Use database estimates if available, otherwise use source path count for initial display
		let (estimated_files, estimated_bytes) = if let Some(ref estimates) = db_estimates {
			if estimates.is_complete() {
				// We have complete information from database
				(estimates.file_count as usize, estimates.total_size)
			} else {
				// Partial information - still useful for initial display
				(
					self.sources.paths.len().max(estimates.file_count as usize),
					estimates.total_size,
				)
			}
		} else {
			(self.sources.paths.len(), 0)
		};

		// Phase 3: Preparation - Calculate actual total size
		let progress = CopyProgress {
			phase: CopyPhase::Preparation,
			current_file: String::new(),
			files_copied: 0,
			total_files: estimated_files,
			bytes_copied: 0,
			total_bytes: estimated_bytes,
			current_operation: if db_estimates.is_some() {
				"Verifying file sizes...".to_string()
			} else {
				"Calculating total size...".to_string()
			},
			estimated_remaining: None,
			preparation_complete: false,
			error_count: 0,
		};
		ctx.progress(Progress::generic(progress.to_generic_progress()));

		// Calculate actual file count and total size
		let actual_file_count = self.count_total_files().await?;
		let estimated_total_bytes = self.calculate_total_size(&ctx).await?;

		ctx.log(format!(
			"Preparing to copy {} files ({}) from {} source paths",
			actual_file_count,
			format_bytes(estimated_total_bytes),
			self.sources.paths.len()
		));

		// Update progress with calculated size and file count
		let progress = CopyProgress {
			phase: CopyPhase::Preparation,
			current_file: String::new(),
			files_copied: 0,
			total_files: actual_file_count,
			bytes_copied: 0,
			total_bytes: estimated_total_bytes,
			current_operation: "Preparation complete".to_string(),
			estimated_remaining: None,
			preparation_complete: true,
			error_count: 0,
		};
		ctx.progress(Progress::generic(progress.to_generic_progress()));

		// Create progress aggregator for tracking overall progress
		let mut progress_aggregator =
			ProgressAggregator::new(&ctx, actual_file_count, estimated_total_bytes);

		// Process each source using the appropriate strategy
		for (index, source) in self.sources.paths.iter().enumerate() {
			ctx.check_interrupt().await?;

			// Skip files that have already been completed (resume logic)
			if self.completed_indices.contains(&index) {
				ctx.log(format!(
					"Skipping already completed file: {}",
					source.display()
				));

				// Update progress aggregator to account for already completed files
				let files_in_source = if let Some(local_path) = source.as_local_path() {
					let file_size = self.get_path_size(local_path).await.unwrap_or(0);
					let file_count = self.count_files_in_path(local_path).await.unwrap_or(1);
					progress_aggregator.skip_completed_file(file_size, file_count);
					total_bytes += file_size;
					file_count
				} else {
					1
				};

				copied_count += files_in_source; // Count actual files as copied for progress tracking
				continue;
			}

			let final_destination = if self.sources.paths.len() > 1 {
				// Multiple sources: destination must be a directory
				self.destination
					.join(source.path.file_name().unwrap_or_default())
			} else {
				// Single source: check if destination is a directory
				if let Some(dest_path) = self.destination.as_local_path() {
					if dest_path.is_dir() {
						// Destination is a directory, join with source filename
						self.destination
							.join(source.path.file_name().unwrap_or_default())
					} else {
						// Destination is a file path, use as-is
						self.destination.clone()
					}
				} else {
					// Non-local destination, assume file copy
					self.destination.clone()
				}
			};

			// Count files in this source path for accurate progress tracking
			let files_in_source = if let Some(local_path) = source.as_local_path() {
				self.count_files_in_path(local_path).await.unwrap_or(1)
			} else {
				1
			};

			// Update aggregator with current file info
			let operation_description = CopyStrategyRouter::describe_strategy(
				source,
				&final_destination,
				is_move,
				&self.options.copy_method,
				volume_manager.as_deref(),
			)
			.await;

			progress_aggregator.start_file(source.display(), operation_description);
			progress_aggregator.set_error_count(failed_copies.len());

			// Update progress - show files already completed
			let files_completed_count = *progress_aggregator.files_completed.lock().unwrap();
			let bytes_completed_snapshot = *progress_aggregator
				.bytes_completed_before_current
				.lock()
				.unwrap();
			let progress = CopyProgress {
				phase: CopyPhase::Copying,
				current_file: source.display(),
				files_copied: files_completed_count,
				total_files: actual_file_count,
				bytes_copied: bytes_completed_snapshot,
				total_bytes: estimated_total_bytes,
				current_operation: progress_aggregator.current_operation.clone(),
				estimated_remaining: None,
				preparation_complete: true,
				error_count: failed_copies.len(),
			};
			ctx.progress(Progress::generic(progress.to_generic_progress()));

			// 1. Select the strategy
			let strategy = CopyStrategyRouter::select_strategy(
				source,
				&final_destination,
				is_move,
				&self.options.copy_method,
				volume_manager.as_deref(),
			)
			.await;

			// 2. Execute the strategy with progress callback
			match strategy
				.execute(
					&ctx,
					source,
					&final_destination,
					self.options.verify_checksum,
					Some(&progress_aggregator.create_callback()),
				)
				.await
			{
				Ok(bytes) => {
					// Mark source as complete (bytes/files already updated by callback)
					progress_aggregator.complete_source();

					// Update totals
					copied_count += files_in_source;
					total_bytes += bytes;

					// Track successful completion for resume
					self.completed_indices.push(index);

					// If this is a move operation and the strategy didn't handle deletion,
					// we need to delete the source after successful copy
					if is_move && source.device_id == final_destination.device_id {
						// For same-device moves, LocalMoveStrategy handles deletion atomically
						// For cross-volume moves, LocalStreamCopyStrategy needs manual deletion
						if let Some(vm) = volume_manager.as_deref() {
							if let (Some(source_path), Some(dest_path)) =
								(source.as_local_path(), final_destination.as_local_path())
							{
								if !vm.same_volume(source_path, dest_path).await {
									// Cross-volume move - delete source
									if let Err(e) = self.delete_source_file(source_path).await {
										failed_copies.push(CopyError {
											source: source.path.clone(),
											destination: final_destination.path.clone(),
											error: format!(
												"Copy succeeded but failed to delete source: {}",
												e
											),
										});
										ctx.add_non_critical_error(format!(
											"Failed to delete source after move {}: {}",
											source.display(),
											e
										));
									}
								}
							}
						}
					}
				}
				Err(e) => {
					failed_copies.push(CopyError {
						source: source.path.clone(),
						destination: final_destination.path.clone(),
						error: e.to_string(),
					});
					ctx.add_non_critical_error(format!(
						"Failed to {} {}: {}",
						if is_move { "move" } else { "copy" },
						source.display(),
						e
					));
				}
			}

			// Checkpoint every 20 files to save completed_indices
			if copied_count % 20 == 0 {
				ctx.checkpoint().await?;
			}
		}

		// Phase 4: Complete
		let progress = CopyProgress {
			phase: CopyPhase::Complete,
			current_file: String::new(),
			files_copied: copied_count,
			total_files: actual_file_count,
			bytes_copied: total_bytes,
			total_bytes: estimated_total_bytes,
			current_operation: "Copy operation complete".to_string(),
			estimated_remaining: None,
			preparation_complete: true,
			error_count: failed_copies.len(),
		};
		ctx.progress(Progress::generic(progress.to_generic_progress()));

		ctx.log(format!(
			"Copy operation completed: {} copied, {} failed",
			copied_count,
			failed_copies.len()
		));

		Ok(FileCopyOutput {
			copied_count,
			failed_count: failed_copies.len(),
			total_bytes,
			duration: self.started_at.elapsed(),
			failed_copies,
			is_move_operation: self.options.delete_after_copy,
		})
	}
}

/// Copy operation phases
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CopyPhase {
	Initializing,
	DatabaseQuery,
	Preparation,
	Copying,
	Complete,
}

impl CopyPhase {
	fn as_str(&self) -> &'static str {
		match self {
			CopyPhase::Initializing => "Initializing",
			CopyPhase::DatabaseQuery => "Database Query",
			CopyPhase::Preparation => "Preparation",
			CopyPhase::Copying => "Copying",
			CopyPhase::Complete => "Complete",
		}
	}
}

/// Copy progress information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyProgress {
	pub phase: CopyPhase,
	pub current_file: String,
	pub files_copied: usize,
	pub total_files: usize,
	pub bytes_copied: u64,
	pub total_bytes: u64,
	pub current_operation: String,
	pub estimated_remaining: Option<Duration>,
	pub preparation_complete: bool,
	pub error_count: usize,
}

impl JobProgress for CopyProgress {}

/// Progress aggregator that tracks overall copy job progress
struct ProgressAggregator<'a> {
	ctx: &'a JobContext<'a>,
	current_file_index: usize,
	total_files: usize,
	bytes_completed_before_current: Arc<Mutex<u64>>,
	total_bytes: u64,
	current_file_path: String,
	current_operation: String,
	error_count: usize,
	files_completed: Arc<Mutex<usize>>,
}

impl<'a> ProgressAggregator<'a> {
	fn new(ctx: &'a JobContext<'a>, total_files: usize, total_bytes: u64) -> Self {
		Self {
			ctx,
			current_file_index: 0,
			total_files,
			bytes_completed_before_current: Arc::new(Mutex::new(0)),
			total_bytes,
			current_file_path: String::new(),
			current_operation: String::new(),
			error_count: 0,
			files_completed: Arc::new(Mutex::new(0)),
		}
	}

	/// Start processing a new file
	fn start_file(&mut self, file_path: String, current_operation: String) {
		self.current_file_path = file_path;
		self.current_operation = current_operation;
	}

	/// Complete the current source item and update index
	fn complete_source(&mut self) {
		self.current_file_index += 1;
		// Note: bytes and file counts are already updated by the callback
	}

	/// Account for files that were already completed (for resume)
	fn skip_completed_file(&mut self, bytes_copied: u64, files_in_operation: usize) {
		*self.bytes_completed_before_current.lock().unwrap() += bytes_copied;
		*self.files_completed.lock().unwrap() += files_in_operation;
	}

	/// Create a progress callback for strategy implementations
	fn create_callback(&self) -> Box<dyn Fn(u64, u64) + Send + Sync + 'a> {
		let ctx = self.ctx;
		let files_completed = self.files_completed.clone();
		let total_files = self.total_files;
		let bytes_before = self.bytes_completed_before_current.clone();
		let total_bytes = self.total_bytes;
		let current_file = self.current_file_path.clone();
		let current_operation = self.current_operation.clone();
		let error_count = self.error_count;

		Box::new(move |bytes_value: u64, signal_value: u64| {
			// NEW SIGNAL: A signal_value of u64::MAX means a file has finished.
			// The bytes_value will be the size of the completed file.
			if signal_value == u64::MAX {
				let mut files = files_completed.lock().unwrap();
				*files += 1;
				let mut bytes = bytes_before.lock().unwrap();
				*bytes += bytes_value; // Add the completed file's size to the total
				ctx.log(format!(
					"File completed. Total files: {}/{}, Total bytes: {}",
					*files, total_files, *bytes
				));
				return;
			}

			// Normal byte-level progress update
			let bytes_before_snapshot = *bytes_before.lock().unwrap();
			let total_bytes_copied = bytes_before_snapshot + bytes_value;
			let files_completed_count = *files_completed.lock().unwrap();

			let copy_progress = CopyProgress {
				phase: CopyPhase::Copying,
				current_file: current_file.clone(),
				files_copied: files_completed_count,
				total_files,
				bytes_copied: total_bytes_copied,
				total_bytes,
				current_operation: current_operation.clone(),
				estimated_remaining: None,
				preparation_complete: true,
				error_count,
			};

			// Log progress details every 100MB
			if total_bytes_copied % (100 * 1024 * 1024) < bytes_value {
				ctx.log(format!(
					"Progress update: {} / {} bytes ({:.1}%)",
					total_bytes_copied,
					total_bytes,
					(total_bytes_copied as f64 / total_bytes as f64) * 100.0
				));
			}

			ctx.progress(Progress::generic(copy_progress.to_generic_progress()));
		})
	}

	fn set_error_count(&mut self, count: usize) {
		self.error_count = count;
	}
}

impl ToGenericProgress for CopyProgress {
	fn to_generic_progress(&self) -> GenericProgress {
		// Calculate percentage based on bytes if available, otherwise use file count
		let percentage = if self.total_bytes > 0 {
			(self.bytes_copied as f32 / self.total_bytes as f32).clamp(0.0, 1.0)
		} else if self.total_files > 0 {
			(self.files_copied as f32 / self.total_files as f32).clamp(0.0, 1.0)
		} else {
			0.0
		};

		// Create appropriate message based on phase
		let message = match self.phase {
			CopyPhase::Initializing => "Initializing copy operation...".to_string(),
			CopyPhase::DatabaseQuery => "Querying database for file information...".to_string(),
			CopyPhase::Preparation => {
				if self.total_files > 0 {
					format!("Preparing to copy {} files...", self.total_files)
				} else {
					"Preparing copy operation...".to_string()
				}
			}
			CopyPhase::Copying => {
				if !self.current_file.is_empty() {
					format!("Copying: {}", self.current_file)
				} else {
					self.current_operation.clone()
				}
			}
			CopyPhase::Complete => format!("Copy complete: {} files", self.files_copied),
		};

		// Build generic progress
		let mut progress = GenericProgress::new(percentage, self.phase.as_str(), message);

		// Only set completion if we're not using byte-based progress
		// (to avoid overriding the percentage)
		if self.total_bytes == 0 {
			progress = progress.with_completion(self.files_copied as u64, self.total_files as u64);
		} else {
			// For byte-based progress, just set the completion counts without recalculating percentage
			progress.completion.completed = self.files_copied as u64;
			progress.completion.total = self.total_files as u64;
		}

		progress = progress.with_bytes(self.bytes_copied, self.total_bytes);

		// Add performance metrics if we're actively copying
		if self.phase == CopyPhase::Copying && self.bytes_copied > 0 {
			// Calculate rate if we have timing information
			// For now, just pass through the estimated remaining time
			progress = progress.with_performance(
				0.0, // Rate will be calculated by job system
				self.estimated_remaining,
				None, // Elapsed time tracked by job system
			);
		}

		// Add error count if any
		if self.error_count > 0 {
			progress = progress.with_errors(self.error_count as u64, 0);
		}

		// Add current file path if available
		if !self.current_file.is_empty() && self.phase == CopyPhase::Copying {
			// Convert current file string to SdPath if possible
			// For now, we'll skip this as we'd need device_id context
		}

		progress
	}
}

/// Error information for failed copies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyError {
	pub source: PathBuf,
	pub destination: PathBuf,
	pub error: String,
}

impl FileCopyJob {
	/// Create a new file copy job with sources and destination
	pub fn new(sources: SdPathBatch, destination: SdPath) -> Self {
		Self {
			sources,
			destination,
			options: Default::default(),
			completed_indices: Vec::new(),
			started_at: Instant::now(),
		}
	}

	/// Create an empty job (used by derive macro)
	pub fn empty() -> Self {
		Self {
			sources: SdPathBatch::new(Vec::new()),
			destination: SdPath::new(uuid::Uuid::new_v4(), PathBuf::new()),
			options: Default::default(),
			completed_indices: Vec::new(),
			started_at: Instant::now(),
		}
	}

	/// Create from individual paths
	pub fn from_paths(sources: Vec<SdPath>, destination: SdPath) -> Self {
		Self::new(SdPathBatch::new(sources), destination)
	}

	/// Set copy options
	pub fn with_options(mut self, options: CopyOptions) -> Self {
		self.options = options;
		self
	}

	/// Create a move job using the copy job with delete_after_copy
	pub fn new_move(sources: SdPathBatch, destination: SdPath, move_mode: MoveMode) -> Self {
		let mut options = CopyOptions::default();
		options.delete_after_copy = true;
		options.move_mode = Some(move_mode);
		Self {
			sources,
			destination,
			options,
			completed_indices: Vec::new(),
			started_at: Instant::now(),
		}
	}

	/// Create a rename operation
	pub fn new_rename(source: SdPath, new_name: String) -> Self {
		let destination = SdPath::new(source.device_id, source.path.with_file_name(new_name));

		Self::new_move(
			SdPathBatch::new(vec![source]),
			destination,
			MoveMode::Rename,
		)
	}

	/// Calculate total size for progress reporting
	async fn calculate_total_size(&self, ctx: &JobContext<'_>) -> JobResult<u64> {
		let mut total = 0u64;

		for source in &self.sources.paths {
			if let Some(local_path) = source.as_local_path() {
				total += self.get_path_size(local_path).await.unwrap_or(0);
			}
		}

		Ok(total)
	}

	/// Count total number of files to be copied (including files within directories)
	async fn count_total_files(&self) -> JobResult<usize> {
		let mut total_count = 0;

		for source in &self.sources.paths {
			if let Some(local_path) = source.as_local_path() {
				total_count += self.count_files_in_path(local_path).await.unwrap_or(0);
			}
		}

		Ok(total_count)
	}

	/// Count files in a path (recursive for directories)
	async fn count_files_in_path(&self, path: &std::path::Path) -> Result<usize, std::io::Error> {
		let mut count = 0;
		let mut stack = vec![path.to_path_buf()];

		while let Some(current_path) = stack.pop() {
			let metadata = tokio::fs::metadata(&current_path).await?;

			if metadata.is_file() {
				count += 1;
			} else if metadata.is_dir() {
				let mut dir = tokio::fs::read_dir(&current_path).await?;
				while let Some(entry) = dir.next_entry().await? {
					stack.push(entry.path());
				}
			}
		}

		Ok(count)
	}

	/// List all files in a directory (recursive)
	async fn list_files_in_directory(&self, path: &std::path::Path) -> JobResult<Vec<PathBuf>> {
		let mut files = Vec::new();
		let mut stack = vec![path.to_path_buf()];

		while let Some(current_path) = stack.pop() {
			let metadata = tokio::fs::metadata(&current_path)
				.await
				.map_err(|e| JobError::execution(format!("Failed to read metadata: {}", e)))?;

			if metadata.is_file() {
				files.push(current_path);
			} else if metadata.is_dir() {
				let mut dir = tokio::fs::read_dir(&current_path)
					.await
					.map_err(|e| JobError::execution(format!("Failed to read directory: {}", e)))?;
				while let Some(entry) = dir.next_entry().await.map_err(|e| {
					JobError::execution(format!("Failed to read directory entry: {}", e))
				})? {
					stack.push(entry.path());
				}
			}
		}

		Ok(files)
	}

	/// Get size of a path (file or directory) using iterative approach
	async fn get_path_size(&self, path: &std::path::Path) -> Result<u64, std::io::Error> {
		let mut total = 0u64;
		let mut stack = vec![path.to_path_buf()];

		while let Some(current_path) = stack.pop() {
			let metadata = tokio::fs::metadata(&current_path).await?;

			if metadata.is_file() {
				total += metadata.len();
			} else if metadata.is_dir() {
				let mut dir = tokio::fs::read_dir(&current_path).await?;
				while let Some(entry) = dir.next_entry().await? {
					stack.push(entry.path());
				}
			}
		}

		Ok(total)
	}

	/// Delete source file after successful cross-volume move
	async fn delete_source_file(&self, source: &std::path::Path) -> Result<(), std::io::Error> {
		let metadata = tokio::fs::metadata(source).await?;

		if metadata.is_file() {
			tokio::fs::remove_file(source).await
		} else if metadata.is_dir() {
			tokio::fs::remove_dir_all(source).await
		} else {
			Ok(())
		}
	}
}

/// Output from file copy job
#[derive(Debug, Serialize, Deserialize)]
pub struct FileCopyOutput {
	pub copied_count: usize,
	pub failed_count: usize,
	pub total_bytes: u64,
	pub duration: Duration,
	pub failed_copies: Vec<CopyError>,
	pub is_move_operation: bool,
}

impl From<FileCopyOutput> for JobOutput {
	fn from(output: FileCopyOutput) -> Self {
		if output.is_move_operation {
			JobOutput::FileMove {
				moved_count: output.copied_count,
				failed_count: output.failed_count,
				total_bytes: output.total_bytes,
			}
		} else {
			JobOutput::FileCopy {
				copied_count: output.copied_count,
				total_bytes: output.total_bytes,
			}
		}
	}
}

/// Backward compatibility wrapper for move operations
#[derive(Debug, Serialize, Deserialize, Job)]
pub struct MoveJob {
	pub sources: SdPathBatch,
	pub destination: SdPath,
	pub mode: MoveMode,
	pub overwrite: bool,
	pub preserve_timestamps: bool,
}

impl Job for MoveJob {
	const NAME: &'static str = "move_files";
	const RESUMABLE: bool = true;
	const DESCRIPTION: Option<&'static str> = Some("Move or rename files and directories");
}

impl crate::infrastructure::jobs::traits::DynJob for MoveJob {
	fn as_any(&self) -> &dyn std::any::Any {
		self
	}

	fn job_name(&self) -> &'static str {
		Self::NAME
	}
}

#[async_trait::async_trait]
impl JobHandler for MoveJob {
	type Output = MoveOutput;

	async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
		// Convert to FileCopyJob with move options
		let mut copy_options = CopyOptions::default();
		copy_options.delete_after_copy = true;
		copy_options.move_mode = Some(self.mode.clone());
		copy_options.overwrite = self.overwrite;
		copy_options.preserve_timestamps = self.preserve_timestamps;

		let mut copy_job = FileCopyJob {
			sources: self.sources.clone(),
			destination: self.destination.clone(),
			options: copy_options,
			completed_indices: Vec::new(),
			started_at: Instant::now(),
		};

		// Run the copy job
		let copy_output = copy_job.run(ctx).await?;

		// Convert output to move format
		Ok(MoveOutput {
			moved_count: copy_output.copied_count,
			failed_count: copy_output.failed_count,
			total_bytes: copy_output.total_bytes,
			duration: copy_output.duration,
			failed_moves: copy_output
				.failed_copies
				.into_iter()
				.map(|e| MoveError {
					source: e.source,
					destination: e.destination,
					error: e.error,
				})
				.collect(),
		})
	}
}

impl MoveJob {
	/// Create a new move job
	pub fn new(sources: SdPathBatch, destination: SdPath, mode: MoveMode) -> Self {
		Self {
			sources,
			destination,
			mode,
			overwrite: false,
			preserve_timestamps: true,
		}
	}

	/// Create an empty job (used by derive macro)
	pub fn empty() -> Self {
		Self {
			sources: SdPathBatch::new(Vec::new()),
			destination: SdPath::new(uuid::Uuid::new_v4(), PathBuf::new()),
			mode: MoveMode::Move,
			overwrite: false,
			preserve_timestamps: true,
		}
	}

	/// Create a rename operation
	pub fn rename(source: SdPath, new_name: String) -> Self {
		let destination = SdPath::new(source.device_id, source.path.with_file_name(new_name));

		Self::new(
			SdPathBatch::new(vec![source]),
			destination,
			MoveMode::Rename,
		)
	}
}

/// Error information for failed moves
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveError {
	pub source: PathBuf,
	pub destination: PathBuf,
	pub error: String,
}

/// Output from move operations
#[derive(Debug, Serialize, Deserialize)]
pub struct MoveOutput {
	pub moved_count: usize,
	pub failed_count: usize,
	pub total_bytes: u64,
	pub duration: Duration,
	pub failed_moves: Vec<MoveError>,
}

impl From<MoveOutput> for JobOutput {
	fn from(output: MoveOutput) -> Self {
		JobOutput::FileMove {
			moved_count: output.moved_count,
			failed_count: output.failed_count,
			total_bytes: output.total_bytes,
		}
	}
}

// Helper function to format bytes in human-readable format
fn format_bytes(bytes: u64) -> String {
	const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
	let mut size = bytes as f64;
	let mut unit_idx = 0;

	while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
		size /= 1024.0;
		unit_idx += 1;
	}

	if unit_idx == 0 {
		format!("{} {}", size as u64, UNITS[unit_idx])
	} else {
		format!("{:.2} {}", size, UNITS[unit_idx])
	}
}

impl Resourceful for FileCopyJob {
	fn get_affected_resources(&self) -> Vec<i32> {
		// FileCopyJob affects files based on SdPaths, but we need entry IDs.
		// This requires database queries to resolve paths to entries.
		// For now, return empty vector - JobManager will handle path-to-entry conversion.
		vec![]
	}
}

impl Resourceful for MoveJob {
	fn get_affected_resources(&self) -> Vec<i32> {
		// MoveJob affects files based on SdPaths, but we need entry IDs.
		// This requires database queries to resolve paths to entries.
		// For now, return empty vector - JobManager will handle path-to-entry conversion.
		vec![]
	}
}
