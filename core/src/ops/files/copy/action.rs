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
			LibraryAction, ValidationResult, ConfirmationRequest,
		},
		job::handle::JobHandle,
	},
};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};
use uuid::Uuid;

/// Internal enum for file conflict resolution strategies
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum FileConflictResolution {
	Overwrite,
	AutoModifyName,
	Abort,
}

impl FileConflictResolution {
	/// All available choices for conflict resolution
	const CHOICES: [Self; 3] = [Self::Overwrite, Self::AutoModifyName, Self::Abort];

	/// Convert to human-readable string
	fn as_str(&self) -> &'static str {
		match self {
			Self::Overwrite => "Overwrite the existing file",
			Self::AutoModifyName => "Rename the new file (e.g., file.txt -> file (1).txt)",
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
		let current = crate::device::get_current_device_id();
		// Sources
		for path in &mut self.input.sources.paths {
			if let crate::domain::addressing::SdPath::Physical { device_id, .. } = path {
				if device_id.is_nil() {
					*device_id = current;
				}
			}
		}
		// Destination
		if let crate::domain::addressing::SdPath::Physical { device_id, .. } = &mut self.input.destination {
			if device_id.is_nil() {
				*device_id = current;
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
			on_conflict: None,
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
	type Output = JobHandle;
	type Input = FileCopyInput;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		use crate::infra::action::builder::ActionBuilder;
		FileCopyActionBuilder::from_input(input)
			.build()
			.map_err(|e| e.to_string())
	}

	async fn execute(
		self,
		library: std::sync::Arc<crate::library::Library>,
		_context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		// Apply conflict resolution to options if set
		let mut options = self.options;
		if let Some(resolution) = self.on_conflict {
			match resolution {
				FileConflictResolution::Overwrite => {
					options.overwrite = true;
				}
				FileConflictResolution::AutoModifyName => {
					// This would be handled by the job's strategy system
					// For now, ensure overwrite is false so the job handles naming
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

		Ok(job_handle)
	}

	fn action_kind(&self) -> &'static str {
		"files.copy"
	}

	async fn validate(
		&self,
		_library: &std::sync::Arc<crate::library::Library>,
		_context: Arc<CoreContext>,
	) -> Result<ValidationResult, ActionError> {
		if self.sources.paths.is_empty() {
			return Err(ActionError::Validation {
				field: "sources".to_string(),
				message: "At least one source file must be specified".to_string(),
			});
		}

		// Check for file conflicts if overwrite is not enabled
		if !self.options.overwrite {
			if let Some(conflict_path) = self.check_for_conflicts().await? {
				let request = ConfirmationRequest {
					message: format!(
						"Destination file already exists: {}",
						conflict_path.display()
					),
					choices: FileConflictResolution::CHOICES
						.iter()
						.map(|c| c.as_str().to_string())
						.collect(),
				};
				return Ok(ValidationResult::RequiresConfirmation(request));
			}
		}

		Ok(ValidationResult::Success)
	}

	fn resolve_confirmation(&mut self, choice_index: usize) -> Result<(), ActionError> {
		match FileConflictResolution::from_index(choice_index) {
			Some(FileConflictResolution::Abort) => {
				Err(ActionError::Cancelled)
			}
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
}

impl FileCopyAction {
	/// Check if any destination files would cause conflicts
	async fn check_for_conflicts(&self) -> Result<Option<PathBuf>, ActionError> {
		// For now, implement a simple check for single file destination conflicts
		// In a full implementation, this would check each source against the destination
		// and handle directory conflicts, etc.
		
		// Extract the physical path from the destination SdPath
		let dest_path = match &self.destination {
			SdPath::Physical { path, .. } => path.clone(),
			SdPath::Virtual { .. } => {
				// Virtual paths would need different conflict resolution
				return Ok(None);
			}
		};

		// Check if destination exists
		if tokio::fs::metadata(&dest_path).await.is_ok() {
			Ok(Some(dest_path))
		} else {
			Ok(None)
		}
	}
}

// Register with the action-centric registry
crate::register_library_action!(FileCopyAction, "files.copy");
