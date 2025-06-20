//! File copy job implementation

use crate::{
	infrastructure::jobs::prelude::*,
	shared::types::{SdPath, SdPathBatch},
};
use serde::{Deserialize, Serialize};
use std::{
	collections::HashMap,
	path::PathBuf,
	time::{Duration, Instant},
};
use tokio::fs;
use uuid::Uuid;

/// Options for file copy operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyOptions {
	pub overwrite: bool,
	pub verify_checksum: bool,
	pub preserve_timestamps: bool,
}

impl Default for CopyOptions {
	fn default() -> Self {
		Self {
			overwrite: false,
			verify_checksum: false,
			preserve_timestamps: true,
		}
	}
}

/// File copy job
#[derive(Debug, Serialize, Deserialize, Job)]
pub struct FileCopyJob {
	pub sources: SdPathBatch,
	pub destination: SdPath,
	#[serde(default)]
	pub options: CopyOptions,

	// Internal state for resumption
	#[serde(skip)]
	completed_indices: Vec<usize>,
	#[serde(skip, default = "Instant::now")]
	started_at: Instant,
}

impl Job for FileCopyJob {
	const NAME: &'static str = "file_copy";
	const RESUMABLE: bool = true;
	const DESCRIPTION: Option<&'static str> = Some("Copy files to a destination");
}

#[async_trait::async_trait]
impl JobHandler for FileCopyJob {
	type Output = FileCopyOutput;

	async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
		ctx.log(format!(
			"Starting copy operation on {} files",
			self.sources.paths.len()
		));

		// Group by device for efficient processing
		let by_device: HashMap<Uuid, Vec<SdPath>> = self
			.sources
			.by_device()
			.into_iter()
			.map(|(device_id, paths)| (device_id, paths.into_iter().cloned().collect()))
			.collect();
		let total_files = self.sources.paths.len();
		let mut copied_count = 0;
		let mut total_bytes = 0u64;
		let mut failed_copies = Vec::new();

		// Calculate total size for progress
		let estimated_total_bytes = self.calculate_total_size(&ctx).await?;

		// Process each device group
		for (device_id, device_paths) in by_device {
			ctx.check_interrupt().await?;

			if device_id == self.destination.device_id {
				// Same device - efficient local copy
				self.process_same_device_copies(
					device_paths.iter().collect(),
					&ctx,
					&mut copied_count,
					&mut total_bytes,
					&mut failed_copies,
					total_files,
					estimated_total_bytes,
				)
				.await?;
			} else {
				// Cross-device copy
				self.process_cross_device_copies(
					device_paths.iter().collect(),
					&ctx,
					&mut copied_count,
					&mut total_bytes,
					&mut failed_copies,
					total_files,
					estimated_total_bytes,
				)
				.await?;
			}
		}

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
		})
	}
}

/// Copy progress information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyProgress {
	pub current_file: String,
	pub files_copied: usize,
	pub total_files: usize,
	pub bytes_copied: u64,
	pub total_bytes: u64,
	pub current_operation: String,
	pub estimated_remaining: Option<Duration>,
}

impl JobProgress for CopyProgress {}

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

	/// Get size of a path (file or directory) using iterative approach
	async fn get_path_size(&self, path: &std::path::Path) -> Result<u64, std::io::Error> {
		let mut total = 0u64;
		let mut stack = vec![path.to_path_buf()];

		while let Some(current_path) = stack.pop() {
			let metadata = fs::metadata(&current_path).await?;

			if metadata.is_file() {
				total += metadata.len();
			} else if metadata.is_dir() {
				let mut dir = fs::read_dir(&current_path).await?;
				while let Some(entry) = dir.next_entry().await? {
					stack.push(entry.path());
				}
			}
		}

		Ok(total)
	}

	/// Process copies within the same device
	async fn process_same_device_copies(
		&mut self,
		paths: Vec<&SdPath>,
		ctx: &JobContext<'_>,
		copied_count: &mut usize,
		total_bytes: &mut u64,
		failed_copies: &mut Vec<CopyError>,
		total_files: usize,
		estimated_total_bytes: u64,
	) -> JobResult<()> {
		for source in paths {
			ctx.check_interrupt().await?;

			if let Some(local_source) = source.as_local_path() {
				let dest_path = self
					.destination
					.path
					.join(local_source.file_name().unwrap_or_default());

				ctx.progress(Progress::structured(CopyProgress {
					current_file: local_source.display().to_string(),
					files_copied: *copied_count,
					total_files,
					bytes_copied: *total_bytes,
					total_bytes: estimated_total_bytes,
					current_operation: "Copying".to_string(),
					estimated_remaining: None,
				}));

				match self.copy_local_file(local_source, &dest_path).await {
					Ok(bytes) => {
						*copied_count += 1;
						*total_bytes += bytes;
						ctx.log(format!(
							"Copied: {} -> {}",
							local_source.display(),
							dest_path.display()
						));
					}
					Err(e) => {
						failed_copies.push(CopyError {
							source: local_source.to_path_buf(),
							destination: dest_path,
							error: e.to_string(),
						});
						ctx.add_non_critical_error(format!(
							"Failed to copy {}: {}",
							local_source.display(),
							e
						));
					}
				}

				// Checkpoint every 20 files
				if *copied_count % 20 == 0 {
					ctx.checkpoint().await?;
				}
			}
		}

		Ok(())
	}

	/// Process cross-device copies
	async fn process_cross_device_copies(
		&mut self,
		paths: Vec<&SdPath>,
		ctx: &JobContext<'_>,
		copied_count: &mut usize,
		total_bytes: &mut u64,
		failed_copies: &mut Vec<CopyError>,
		total_files: usize,
		estimated_total_bytes: u64,
	) -> JobResult<()> {
		for source in paths {
			ctx.check_interrupt().await?;

			ctx.progress(Progress::structured(CopyProgress {
				current_file: source.display(),
				files_copied: *copied_count,
				total_files,
				bytes_copied: *total_bytes,
				total_bytes: estimated_total_bytes,
				current_operation: "Cross-device copy".to_string(),
				estimated_remaining: None,
			}));

			// For cross-device copies, we need to implement network/cloud transfer
			// For now, we'll log that cross-device copy is not yet implemented
			ctx.add_non_critical_error(format!(
				"Cross-device copy not yet implemented for: {}",
				source.display()
			));

			failed_copies.push(CopyError {
				source: source.path.clone(),
				destination: self.destination.path.clone(),
				error: "Cross-device copy not implemented".to_string(),
			});
		}

		Ok(())
	}

	/// Copy a local file or directory
	async fn copy_local_file(
		&self,
		source: &std::path::Path,
		destination: &std::path::Path,
	) -> Result<u64, std::io::Error> {
		// Create destination directory if needed
		if let Some(parent) = destination.parent() {
			fs::create_dir_all(parent).await?;
		}

		// Check if destination exists
		if !self.options.overwrite && fs::try_exists(destination).await? {
			return Err(std::io::Error::new(
				std::io::ErrorKind::AlreadyExists,
				"Destination already exists and overwrite is disabled",
			));
		}

		let metadata = fs::metadata(source).await?;

		if metadata.is_file() {
			let bytes = fs::copy(source, destination).await?;

			// Preserve timestamps if requested
			if self.options.preserve_timestamps {
				if let (Ok(accessed), Ok(modified)) = (metadata.accessed(), metadata.modified()) {
					// Note: Setting timestamps requires platform-specific code
					// This is a simplified version
				}
			}

			Ok(bytes)
		} else if metadata.is_dir() {
			self.copy_directory_recursive(source, destination).await
		} else {
			Ok(0)
		}
	}

	/// Copy a directory using iterative approach
	async fn copy_directory_recursive(
		&self,
		source: &std::path::Path,
		destination: &std::path::Path,
	) -> Result<u64, std::io::Error> {
		fs::create_dir_all(destination).await?;
		let mut total_size = 0u64;
		let mut stack = vec![(source.to_path_buf(), destination.to_path_buf())];

		while let Some((src_path, dest_path)) = stack.pop() {
			if src_path.is_file() {
				total_size += fs::copy(&src_path, &dest_path).await?;
			} else if src_path.is_dir() {
				fs::create_dir_all(&dest_path).await?;
				let mut dir = fs::read_dir(&src_path).await?;

				while let Some(entry) = dir.next_entry().await? {
					let entry_src = entry.path();
					let entry_dest = dest_path.join(entry.file_name());
					stack.push((entry_src, entry_dest));
				}
			}
		}

		Ok(total_size)
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
}

impl From<FileCopyOutput> for JobOutput {
	fn from(output: FileCopyOutput) -> Self {
		JobOutput::FileCopy {
			copied_count: output.copied_count,
			total_bytes: output.total_bytes,
		}
	}
}

// Job registration is now handled automatically by the derive macro
