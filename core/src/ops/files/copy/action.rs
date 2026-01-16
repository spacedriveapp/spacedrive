//! File copy action handler

use super::{
	input::FileCopyInput,
	job::{CopyOptions, FileCopyJob},
};
use crate::{
	context::CoreContext,
	domain::addressing::{SdPath, SdPathBatch},
	infra::{
		action::{
			builder::{ActionBuildError, ActionBuilder},
			error::ActionError,
			ConfirmationRequest, LibraryAction, ValidationResult,
		},
		job::handle::JobHandle,
	},
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{path::PathBuf, sync::Arc};
use uuid::Uuid;

/// Internal enum for file conflict resolution strategies
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum FileConflictResolution {
	Overwrite,
	AutoModifyName,
	Skip,
	Abort,
}

impl FileConflictResolution {
	/// All available choices for conflict resolution
	const CHOICES: [Self; 4] = [
		Self::Overwrite,
		Self::AutoModifyName,
		Self::Skip,
		Self::Abort,
	];

	/// Convert to human-readable string
	fn as_str(&self) -> &'static str {
		match self {
			Self::Overwrite => "Overwrite the existing file",
			Self::AutoModifyName => "Rename the new file (e.g., file.txt -> file (1).txt)",
			Self::Skip => "Skip files that already exist",
			Self::Abort => "Abort this copy operation",
		}
	}

	/// Create from choice index
	fn from_index(index: usize) -> Option<Self> {
		Self::CHOICES.get(index).copied()
	}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileCopyAction {
	pub sources: SdPathBatch,
	pub destination: SdPath,
	pub options: CopyOptions,
	/// Conflict resolution strategy set after user confirmation
	#[serde(skip_serializing_if = "Option::is_none")]
	pub on_conflict: Option<FileConflictResolution>,
}

/// Builder for creating FileCopyAction instances with fluent API
#[derive(Debug, Clone)]
pub struct FileCopyActionBuilder {
	input: FileCopyInput,
	errors: Vec<String>,
}

impl FileCopyActionBuilder {
	/// Create a new builder
	pub fn new() -> Self {
		Self {
			input: FileCopyInput::default(),
			errors: Vec::new(),
		}
	}

	/// Create builder from core input type (primary interface)
	pub fn from_input(input: FileCopyInput) -> Self {
		Self {
			input,
			errors: Vec::new(),
		}
	}

	/// Add multiple local source files
	pub fn sources<I, P>(mut self, sources: I) -> Self
	where
		I: IntoIterator<Item = P>,
		P: Into<PathBuf>,
	{
		let paths: Vec<SdPath> = sources
			.into_iter()
			.map(|p| SdPath::local(p.into()))
			.collect();
		self.input.sources.extend(paths);
		self
	}

	/// Add a single local source file
	pub fn source<P: Into<PathBuf>>(mut self, source: P) -> Self {
		self.input.sources.paths.push(SdPath::local(source.into()));
		self
	}

	/// Set the local destination path
	pub fn destination<P: Into<PathBuf>>(mut self, dest: P) -> Self {
		self.input.destination = SdPath::local(dest.into());
		self
	}

	/// Set whether to overwrite existing files
	pub fn overwrite(mut self, overwrite: bool) -> Self {
		self.input.overwrite = overwrite;
		self
	}

	/// Set whether to verify checksums during copy
	pub fn verify_checksum(mut self, verify: bool) -> Self {
		self.input.verify_checksum = verify;
		self
	}

	/// Set whether to preserve file timestamps
	pub fn preserve_timestamps(mut self, preserve: bool) -> Self {
		self.input.preserve_timestamps = preserve;
		self
	}

	/// Enable move mode (delete source after copy)
	pub fn move_files(mut self, enable: bool) -> Self {
		self.input.move_files = enable;
		self
	}

	/// Validate sources exist and are readable
	fn validate_sources(&mut self) {
		// Normalize any nil-device local paths to the current device before validation
		self.normalize_local_device_ids();
		// First do basic validation from input
		if let Err(basic_errors) = self.input.validate() {
			self.errors.extend(basic_errors);
			return;
		}

		// Then do filesystem validation for local paths only
		for source in &self.input.sources.paths {
			if let Some(local_path) = source.as_local_path() {
				if !local_path.exists() {
					self.errors.push(format!(
						"Source file does not exist: {}",
						local_path.display()
					));
				} else if local_path.is_dir() && std::fs::read_dir(local_path).is_err() {
					self.errors
						.push(format!("Cannot read directory: {}", local_path.display()));
				} else if local_path.is_file() && std::fs::metadata(local_path).is_err() {
					self.errors
						.push(format!("Cannot access file: {}", local_path.display()));
				}
			}
		}
	}

	/// Validate destination is valid
	fn validate_destination(&mut self) {
		// Ensure destination device id is normalized for local paths
		self.normalize_local_device_ids();
		if let Some(dest_path) = self.input.destination.as_local_path() {
			if let Some(parent) = dest_path.parent() {
				if !parent.exists() {
					self.errors.push(format!(
						"Destination directory does not exist: {}",
						parent.display()
					));
				}
			}
		}
	}

	/// Replace nil device IDs on Physical paths with the daemon's current device ID
	fn normalize_local_device_ids(&mut self) {
		let current_slug = crate::device::get_current_device_slug();
		// Sources
		for path in &mut self.input.sources.paths {
			if let crate::domain::addressing::SdPath::Physical { device_slug, .. } = path {
				if device_slug.is_empty() {
					*device_slug = current_slug.clone();
				}
			}
		}
		// Destination
		if let crate::domain::addressing::SdPath::Physical { device_slug, .. } =
			&mut self.input.destination
		{
			if device_slug.is_empty() {
				*device_slug = current_slug;
			}
		}
	}
}

impl ActionBuilder for FileCopyActionBuilder {
	type Action = FileCopyAction;
	type Error = ActionBuildError;

	fn validate(&self) -> Result<(), Self::Error> {
		let mut builder = self.clone();
		builder.normalize_local_device_ids();
		builder.validate_sources();
		builder.validate_destination();

		if !builder.errors.is_empty() {
			return Err(ActionBuildError::validations(builder.errors));
		}

		Ok(())
	}

	fn build(self) -> Result<Self::Action, Self::Error> {
		self.validate()?;

		let mut this = self;
		this.normalize_local_device_ids();

		let options = this.input.to_copy_options();

		Ok(FileCopyAction {
			sources: this.input.sources,
			destination: this.input.destination,
			options,
			on_conflict: this.input.on_conflict,
		})
	}
}

/// Convenience methods on FileCopyAction
impl FileCopyAction {
	/// Create a new builder
	pub fn builder() -> FileCopyActionBuilder {
		FileCopyActionBuilder::new()
	}

	/// Quick builder for copying a single file
	pub fn copy_file<S: Into<PathBuf>, D: Into<PathBuf>>(
		source: S,
		dest: D,
	) -> FileCopyActionBuilder {
		FileCopyActionBuilder::new()
			.source(source)
			.destination(dest)
	}

	/// Quick builder for copying multiple files
	pub fn copy_files<I, P, D>(sources: I, dest: D) -> FileCopyActionBuilder
	where
		I: IntoIterator<Item = P>,
		P: Into<PathBuf>,
		D: Into<PathBuf>,
	{
		FileCopyActionBuilder::new()
			.sources(sources)
			.destination(dest)
	}
}

// Legacy handler removed; action is registered via the new action-centric registry

impl LibraryAction for FileCopyAction {
	type Output = crate::infra::job::handle::JobReceipt;
	type Input = FileCopyInput;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		use crate::infra::action::builder::ActionBuilder;
		FileCopyActionBuilder::from_input(input)
			.build()
			.map_err(|e| e.to_string())
	}

	async fn validate(
		&self,
		_library: &std::sync::Arc<crate::library::Library>,
		context: Arc<CoreContext>,
	) -> Result<ValidationResult, ActionError> {
		use serde_json::json;

		if self.sources.paths.is_empty() {
			return Err(ActionError::Validation {
				field: "sources".to_string(),
				message: "At least one source file must be specified".to_string(),
			});
		}

		// Get strategy metadata for rich UI display
		let first_source = &self.sources.paths[0];
		let (_, strategy_metadata) =
			super::routing::CopyStrategyRouter::select_strategy_with_metadata(
				first_source,
				&self.destination,
				self.options.delete_after_copy,
				&self.options.copy_method,
				Some(&*context.volume_manager),
			)
			.await;

		// Calculate file counts and total bytes
		let (file_count, total_bytes) = self.calculate_totals().await?;

		// Check for file conflicts if overwrite is not enabled AND on_conflict is not already set
		// If on_conflict is set, the user has already made their choice (via UI or CLI)
		if !self.options.overwrite && self.on_conflict.is_none() {
			let conflicts = self.check_for_conflicts_detailed().await?;

			if !conflicts.is_empty() {
				let metadata = json!({
					"strategy": strategy_metadata,
					"file_count": file_count,
					"total_bytes": total_bytes,
					"conflicts": conflicts.iter().map(|(source, dest)| {
						json!({
							"source": source.to_string_lossy(),
							"destination": dest.to_string_lossy(),
						})
					}).collect::<Vec<_>>(),
					"is_fast_operation": strategy_metadata.is_fast_operation,
				});

				let request = ConfirmationRequest {
					message: format!(
						"{} file conflict{} detected",
						conflicts.len(),
						if conflicts.len() == 1 { "" } else { "s" }
					),
					choices: FileConflictResolution::CHOICES
						.iter()
						.map(|c| c.as_str().to_string())
						.collect(),
					metadata: Some(metadata),
				};
				return Ok(ValidationResult::RequiresConfirmation(request));
			}
		}

		// No conflicts - return success with metadata for auto-proceed decision
		let metadata = json!({
			"strategy": strategy_metadata,
			"file_count": file_count,
			"total_bytes": total_bytes,
			"conflicts": [],
			"is_fast_operation": strategy_metadata.is_fast_operation,
		});

		// If it's a fast operation with no conflicts, return success with metadata
		// Frontend can use this to decide whether to show a modal or auto-proceed
		Ok(ValidationResult::Success {
			metadata: Some(metadata),
		})
	}

	fn resolve_confirmation(&mut self, choice_index: usize) -> Result<(), ActionError> {
		match FileConflictResolution::from_index(choice_index) {
			Some(FileConflictResolution::Abort) => Err(ActionError::Cancelled),
			Some(resolution) => {
				self.on_conflict = Some(resolution);
				Ok(())
			}
			None => Err(ActionError::Validation {
				field: "choice".to_string(),
				message: "Invalid choice selected".to_string(),
			}),
		}
	}

	async fn execute(
		mut self,
		library: std::sync::Arc<crate::library::Library>,
		_context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		// Apply conflict resolution to options
		let mut options = self.options.clone();

		// Pass the conflict resolution to the job
		options.conflict_resolution = self.on_conflict;

		// Set overwrite flag based on resolution
		if let Some(resolution) = self.on_conflict {
			match resolution {
				FileConflictResolution::Overwrite => {
					options.overwrite = true;
				}
				FileConflictResolution::AutoModifyName | FileConflictResolution::Skip => {
					// These are handled per-file in the job
					options.overwrite = false;
				}
				FileConflictResolution::Abort => {
					// This should have been handled in resolve_confirmation
					return Err(ActionError::Cancelled);
				}
			}
		}

		let job = FileCopyJob::new(self.sources, self.destination).with_options(options);

		let job_handle = library
			.jobs()
			.dispatch(job)
			.await
			.map_err(ActionError::Job)?;

		Ok(job_handle.into())
	}

	fn action_kind(&self) -> &'static str {
		"files.copy"
	}
}

impl FileCopyAction {
	/// Calculate total file count and bytes for the sources
	async fn calculate_totals(&self) -> Result<(usize, u64), ActionError> {
		let mut total_files = 0usize;
		let mut total_bytes = 0u64;

		for source in &self.sources.paths {
			if let Some(local_path) = source.as_local_path() {
				let metadata = tokio::fs::metadata(local_path).await.map_err(|e| {
					ActionError::Internal(format!("Failed to read metadata: {}", e))
				})?;

				if metadata.is_file() {
					total_files += 1;
					total_bytes += metadata.len();
				} else if metadata.is_dir() {
					let (count, size) = self.count_directory(local_path).await?;
					total_files += count;
					total_bytes += size;
				}
			}
		}

		Ok((total_files, total_bytes))
	}

	/// Count files and total size in a directory recursively
	async fn count_directory(&self, path: &std::path::Path) -> Result<(usize, u64), ActionError> {
		let mut count = 0usize;
		let mut size = 0u64;
		let mut stack = vec![path.to_path_buf()];

		while let Some(current) = stack.pop() {
			let metadata = tokio::fs::metadata(&current)
				.await
				.map_err(|e| ActionError::Internal(format!("Failed to read metadata: {}", e)))?;

			if metadata.is_file() {
				count += 1;
				size += metadata.len();
			} else if metadata.is_dir() {
				let mut dir = tokio::fs::read_dir(&current).await.map_err(|e| {
					ActionError::Internal(format!("Failed to read directory: {}", e))
				})?;

				while let Some(entry) = dir.next_entry().await.map_err(|e| {
					ActionError::Internal(format!("Failed to read directory entry: {}", e))
				})? {
					stack.push(entry.path());
				}
			}
		}

		Ok((count, size))
	}

	/// Check for all file conflicts and return list of conflicting (source, destination) pairs
	async fn check_for_conflicts_detailed(&self) -> Result<Vec<(PathBuf, PathBuf)>, ActionError> {
		let mut conflicts = Vec::new();

		let dest_path = match self.destination.as_local_path() {
			Some(p) => p,
			None => return Ok(conflicts), // Non-local destinations don't have conflicts yet
		};

		// Check if destination is a directory
		let dest_is_dir = dest_path.is_dir();

		for source in &self.sources.paths {
			if let Some(source_path) = source.as_local_path() {
				// Calculate the actual destination path for this source
				let actual_dest = if dest_is_dir || self.sources.paths.len() > 1 {
					if let Some(filename) = source_path.file_name() {
						dest_path.join(filename)
					} else {
						continue;
					}
				} else {
					dest_path.to_path_buf()
				};

				// Check if this would conflict
				if actual_dest.exists() {
					conflicts.push((source_path.to_path_buf(), actual_dest));
				}
			}
		}

		Ok(conflicts)
	}

	/// Check if any destination files would cause conflicts (legacy method)
	async fn check_for_conflicts(&self) -> Result<Option<PathBuf>, ActionError> {
		let conflicts = self.check_for_conflicts_detailed().await?;
		Ok(conflicts.into_iter().next().map(|(_, dest)| dest))
	}

	/// Generate a unique destination path by appending a number if the original exists
	async fn generate_unique_destination(&self) -> Result<SdPath, ActionError> {
		use std::path::Path;

		let SdPath::Physical { device_slug, path } = &self.destination else {
			// For non-physical paths, just return the original
			return Ok(self.destination.clone());
		};

		// First, resolve the actual destination path using the same logic as the job
		let resolved_destination = self.resolve_final_destination_path(path)?;

		let mut counter = 1;
		let mut new_path = resolved_destination.clone();

		// Keep trying until we find a path that doesn't exist
		while tokio::fs::metadata(&new_path).await.is_ok() {
			// Generate new name with counter
			if let Some(parent) = resolved_destination.parent() {
				if let Some(file_name) = resolved_destination.file_name() {
					let file_name_str = file_name.to_string_lossy();

					// Split filename and extension
					if let Some(dot_pos) = file_name_str.rfind('.') {
						let name = &file_name_str[..dot_pos];
						let ext = &file_name_str[dot_pos..];
						new_path = parent.join(format!("{} ({}){}", name, counter, ext));
					} else {
						// No extension
						new_path = parent.join(format!("{} ({})", file_name_str, counter));
					}
				} else {
					// Fallback if we can't get filename
					new_path = resolved_destination.with_file_name(format!("copy_{}", counter));
				}
			} else {
				// Fallback if we can't get parent
				new_path = Path::new(&format!(
					"{}_copy_{}",
					resolved_destination.display(),
					counter
				))
				.to_path_buf();
			}

			counter += 1;

			// Safety check to avoid infinite loops
			if counter > 1000 {
				return Err(ActionError::Internal(
					"Could not generate unique filename after 1000 attempts".to_string(),
				));
			}
		}

		Ok(SdPath::Physical {
			device_slug: device_slug.clone(),
			path: new_path,
		})
	}

	/// Resolve the final destination path using the same logic as the job
	/// This handles the case where destination is a directory vs a file path
	fn resolve_final_destination_path(
		&self,
		dest_path: &std::path::PathBuf,
	) -> Result<std::path::PathBuf, ActionError> {
		if self.sources.paths.len() > 1 {
			// Multiple sources: destination must be a directory
			if let Some(first_source) = self.sources.paths.first() {
				if let SdPath::Physical {
					path: source_path, ..
				} = first_source
				{
					if let Some(filename) = source_path.file_name() {
						return Ok(dest_path.join(filename));
					}
				}
			}
			// Fallback
			return Ok(dest_path.clone());
		} else {
			// Single source: check if destination is a directory
			if dest_path.is_dir() {
				// Destination is a directory, join with source filename
				if let Some(source) = self.sources.paths.first() {
					if let SdPath::Physical {
						path: source_path, ..
					} = source
					{
						if let Some(filename) = source_path.file_name() {
							return Ok(dest_path.join(filename));
						}
					}
				}
				// Fallback
				return Ok(dest_path.clone());
			} else {
				// Destination is a file path, use as-is
				return Ok(dest_path.clone());
			}
		}
	}
}

// Register with the action-centric registry
crate::register_library_action!(FileCopyAction, "files.copy");
