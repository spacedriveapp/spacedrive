//! File copy action handler

use super::{
	input::FileCopyInput,
	job::{CopyOptions, FileCopyJob},
	output::FileCopyActionOutput,
};
use crate::{
	context::CoreContext,
	domain::addressing::{SdPath, SdPathBatch},
	infra::{
		action::{
			builder::{ActionBuildError, ActionBuilder},
			error::{ActionError, ActionResult},
			LibraryAction,
		},
		job::handle::JobHandle,
	},
};
use async_trait::async_trait;
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileCopyAction {
	pub sources: Vec<SdPath>,
	pub destination: SdPath,
	pub options: CopyOptions,
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

	/// Add multiple source files
	pub fn sources<I, P>(mut self, sources: I) -> Self
	where
		I: IntoIterator<Item = P>,
		P: Into<PathBuf>,
	{
		self.input
			.sources
			.extend(sources.into_iter().map(|p| p.into()));
		self
	}

	/// Add a single source file
	pub fn source<P: Into<PathBuf>>(mut self, source: P) -> Self {
		self.input.sources.push(source.into());
		self
	}

	/// Set the destination path
	pub fn destination<P: Into<PathBuf>>(mut self, dest: P) -> Self {
		self.input.destination = dest.into();
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
		// First do basic validation from input
		if let Err(basic_errors) = self.input.validate() {
			self.errors.extend(basic_errors);
			return;
		}

		// Then do filesystem validation
		for source in &self.input.sources {
			if !source.exists() {
				self.errors
					.push(format!("Source file does not exist: {}", source.display()));
			} else if source.is_dir() && !source.read_dir().is_ok() {
				self.errors
					.push(format!("Cannot read directory: {}", source.display()));
			} else if source.is_file() && std::fs::metadata(source).is_err() {
				self.errors
					.push(format!("Cannot access file: {}", source.display()));
			}
		}
	}

	/// Validate destination is valid
	fn validate_destination(&mut self) {
		if let Some(parent) = self.input.destination.parent() {
			if !parent.exists() {
				self.errors.push(format!(
					"Destination directory does not exist: {}",
					parent.display()
				));
			}
		}
	}
}

impl ActionBuilder for FileCopyActionBuilder {
	type Action = FileCopyAction;
	type Error = ActionBuildError;

	fn validate(&self) -> Result<(), Self::Error> {
		let mut builder = self.clone();
		builder.validate_sources();
		builder.validate_destination();

		if !builder.errors.is_empty() {
			return Err(ActionBuildError::validations(builder.errors));
		}

		Ok(())
	}

	fn build(self) -> Result<Self::Action, Self::Error> {
		self.validate()?;

		let options = self.input.to_copy_options();

		// Convert PathBuf to SdPath (local paths)
		let sources = self
			.input
			.sources
			.iter()
			.map(|p| SdPath::local(p))
			.collect();
		let destination = SdPath::local(&self.input.destination);

		Ok(FileCopyAction {
			sources,
			destination,
			options,
		})
	}
}

impl FileCopyActionBuilder {
	/// Create action directly from URI strings (for CLI/API use)
	pub fn from_uris(
		source_uris: Vec<String>,
		destination_uri: String,
		options: CopyOptions,
	) -> Result<FileCopyAction, ActionBuildError> {
		// Parse source URIs to SdPaths
		let mut sources = Vec::new();
		for uri in source_uris {
			match SdPath::from_uri(&uri) {
				Ok(path) => sources.push(path),
				Err(e) => {
					return Err(ActionBuildError::validation(format!(
						"Invalid source URI '{}': {:?}",
						uri, e
					)))
				}
			}
		}

		// Parse destination URI
		let destination = SdPath::from_uri(&destination_uri).map_err(|e| {
			ActionBuildError::validation(format!(
				"Invalid destination URI '{}': {:?}",
				destination_uri, e
			))
		})?;

		Ok(FileCopyAction {
			sources,
			destination,
			options,
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

pub struct FileCopyHandler;

impl FileCopyHandler {
	pub fn new() -> Self {
		Self
	}
}

// Implement the unified ActionTrait (replaces ActionHandler)
impl LibraryAction for FileCopyAction {
	type Output = JobHandle;

	async fn execute(
		self,
		library: std::sync::Arc<crate::library::Library>,
		context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		// Library is pre-validated by ActionManager - no boilerplate!

		// Create job instance directly
		let job = FileCopyJob::new(SdPathBatch::new(self.sources), self.destination)
			.with_options(self.options);

		// Dispatch job and return handle directly - no string conversion!
		let job_handle = library
			.jobs()
			.dispatch(job)
			.await
			.map_err(ActionError::Job)?;

		Ok(job_handle)
	}

	fn action_kind(&self) -> &'static str {
		"file.copy"
	}

	async fn validate(
		&self,
		library: &std::sync::Arc<crate::library::Library>,
		context: Arc<CoreContext>,
	) -> Result<(), ActionError> {
		if self.sources.is_empty() {
			return Err(ActionError::Validation {
				field: "sources".to_string(),
				message: "At least one source file must be specified".to_string(),
			});
		}
		Ok(())
	}
}
