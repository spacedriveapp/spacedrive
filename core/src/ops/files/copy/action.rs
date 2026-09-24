//! File copy action handler

use super::{
	input::FileCopyInput,
	job::{CopyOptions, FileCopyJob},
};
use crate::{
	context::CoreContext,
	domain::addressing::{SdPath, SdPathBatch},
	infra::action::{
		builder::{ActionBuildError, ActionBuilder},
		error::ActionError,
		LibraryAction,
	},
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{path::PathBuf, sync::Arc};

/// Internal enum for file conflict resolution strategies
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum FileConflictResolution {
	Overwrite,
	AutoModifyName,
	Skip,
	Abort,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileCopyAction {
	pub sources: SdPathBatch,
	pub destination: SdPath,
	pub options: CopyOptions,
	/// How a colliding file is handled, when the caller chose
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

impl LibraryAction for FileCopyAction {
	type Output = crate::infra::job::handle::JobReceipt;
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
		let mut options = self.options.clone();
		options.conflict_resolution = self.on_conflict;
		match self.on_conflict {
			Some(FileConflictResolution::Overwrite) => options.overwrite = true,
			Some(FileConflictResolution::AutoModifyName | FileConflictResolution::Skip) => {
				options.overwrite = false
			}
			Some(FileConflictResolution::Abort) => return Err(ActionError::Cancelled),
			None => {}
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

// Register with the action-centric registry
crate::register_library_action!(FileCopyAction, "files.copy");
