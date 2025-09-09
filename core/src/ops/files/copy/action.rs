//! File copy action handler

use super::{
	input::FileCopyInput,
	job::{CopyOptions, FileCopyJob},
	output::FileCopyActionOutput,
};
use crate::{
	context::CoreContext,
	infra::{
		action::{
			builder::{ActionBuildError, ActionBuilder},
			error::{ActionError, ActionResult},
			LibraryAction,
		},
		cli::adapters::FileCopyCliArgs,
		job::handle::JobHandle,
	},
	domain::addressing::{SdPath, SdPathBatch},
};
use async_trait::async_trait;
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileCopyAction {
	pub library_id: Uuid,
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

	/// Set the library ID for this operation
	pub fn library_id(mut self, library_id: uuid::Uuid) -> Self {
		self.input.library_id = Some(library_id);
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
		let sources = self.input.sources.iter()
			.map(|p| SdPath::local(p))
			.collect();
		let destination = SdPath::local(&self.input.destination);

		Ok(FileCopyAction {
			library_id: self.input.library_id.ok_or_else(|| ActionBuildError::validation("library_id is required".to_string()))?,
			sources,
			destination,
			options,
		})
	}
}

impl FileCopyActionBuilder {
	/// Create builder from CLI args (interface-specific convenience method)
	pub fn from_cli_args(args: FileCopyCliArgs) -> Self {
		Self::from_input(args.into())
	}

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
				Err(e) => return Err(ActionBuildError::validation(
					format!("Invalid source URI '{}': {:?}", uri, e)
				)),
			}
		}

		// Parse destination URI
		let destination = SdPath::from_uri(&destination_uri)
			.map_err(|e| ActionBuildError::validation(
				format!("Invalid destination URI '{}': {:?}", destination_uri, e)
			))?;

		Ok(FileCopyAction {
			library_id: Uuid::nil(),
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

	async fn execute(self, library: std::sync::Arc<crate::library::Library>, context: Arc<CoreContext>) -> Result<Self::Output, ActionError> {
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

	fn library_id(&self) -> Uuid {
		self.library_id
	}

	async fn validate(&self, library: &std::sync::Arc<crate::library::Library>, context: Arc<CoreContext>) -> Result<(), ActionError> {
		if self.sources.is_empty() {
			return Err(ActionError::Validation {
				field: "sources".to_string(),
				message: "At least one source file must be specified".to_string(),
			});
		}
		Ok(())
	}
}

// ActionHandler removed - using unified ActionTrait instead

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{
		infra::cli::adapters::{copy::CopyMethodCli, FileCopyCliArgs},
		ops::files::input::CopyMethod,
	};
	use std::path::PathBuf;

	#[test]
	fn test_builder_fluent_api() {
		let action = FileCopyAction::builder()
			.sources(["/src/file1.txt", "/src/file2.txt"])
			.destination("/dest/")
			.overwrite(true)
			.verify_checksum(true)
			.preserve_timestamps(false)
			.move_files(true)
			.build();

		// Note: This will fail validation because files don't exist, but it tests the API
		assert!(action.is_err());
		match action.unwrap_err() {
			ActionBuildError::Validation(errors) => {
				assert!(!errors.is_empty());
				assert!(errors.iter().any(|e| e.contains("does not exist")));
			}
			_ => panic!("Expected validation error"),
		}
	}

	#[test]
	fn test_builder_validation_empty_sources() {
		let result = FileCopyAction::builder().destination("/dest/").build();

		assert!(result.is_err());
		match result.unwrap_err() {
			ActionBuildError::Validation(errors) => {
				assert!(errors.iter().any(|e| e.contains("At least one source")));
			}
			_ => panic!("Expected validation error"),
		}
	}

	#[test]
	fn test_builder_from_input() {
		let input = FileCopyInput::new(vec!["/file1.txt".into(), "/file2.txt".into()], "/dest/")
			.with_overwrite(true)
			.with_verification(true)
			.with_move(false);

		let builder = FileCopyActionBuilder::from_input(input.clone());

		// Test that builder has correct values from input
		assert_eq!(
			builder.input.sources,
			vec![PathBuf::from("/file1.txt"), PathBuf::from("/file2.txt")]
		);
		assert_eq!(builder.input.destination, PathBuf::from("/dest/"));
		assert!(builder.input.overwrite);
		assert!(builder.input.verify_checksum);
		assert!(!builder.input.move_files);
	}

	#[test]
	fn test_cli_integration() {
		let args = FileCopyCliArgs {
			sources: vec!["/src/file.txt".into()],
			destination: "/dest/".into(),
			method: CopyMethodCli::Auto,
			overwrite: true,
			verify: false,
			preserve_timestamps: true,
			move_files: false,
		};

		let builder = FileCopyActionBuilder::from_cli_args(args);

		// Test that builder has correct values set from CLI args
		assert_eq!(builder.input.sources, vec![PathBuf::from("/src/file.txt")]);
		assert_eq!(builder.input.destination, PathBuf::from("/dest/"));
		assert!(builder.input.overwrite);
		assert!(!builder.input.verify_checksum);
		assert!(builder.input.preserve_timestamps);
		assert!(!builder.input.move_files);
	}

	#[test]
	fn test_convenience_methods() {
		// Test single file copy
		let builder = FileCopyAction::copy_file("/src/file.txt", "/dest/file.txt");
		assert_eq!(builder.input.sources, vec![PathBuf::from("/src/file.txt")]);
		assert_eq!(builder.input.destination, PathBuf::from("/dest/file.txt"));

		// Test multiple files copy
		let sources = vec!["/src/file1.txt", "/src/file2.txt"];
		let builder = FileCopyAction::copy_files(sources.clone(), "/dest/");
		assert_eq!(
			builder.input.sources,
			sources.into_iter().map(PathBuf::from).collect::<Vec<_>>()
		);
		assert_eq!(builder.input.destination, PathBuf::from("/dest/"));
	}

	#[test]
	fn test_builder_chaining() {
		let builder = FileCopyAction::builder()
			.source("/file1.txt")
			.source("/file2.txt")
			.source("/file3.txt")
			.destination("/dest/")
			.overwrite(true)
			.verify_checksum(false)
			.preserve_timestamps(true)
			.move_files(false);

		assert_eq!(builder.input.sources.len(), 3);
		assert!(builder.input.overwrite);
		assert!(!builder.input.verify_checksum);
		assert!(builder.input.preserve_timestamps);
		assert!(!builder.input.move_files);
	}

	#[test]
	fn test_input_abstraction_flow() {
		// Test the full flow: CLI args -> Input -> Builder -> Action
		let cli_args = FileCopyCliArgs {
			sources: vec!["/source.txt".into()],
			destination: "/dest.txt".into(),
			method: CopyMethodCli::Auto,
			overwrite: false,
			verify: true,
			preserve_timestamps: false,
			move_files: true,
		};

		// Convert CLI args to input
		let input: FileCopyInput = cli_args.into();
		assert_eq!(input.sources, vec![PathBuf::from("/source.txt")]);
		assert!(input.verify_checksum);
		assert!(!input.preserve_timestamps);
		assert!(input.move_files);

		// Create builder from input
		let builder = FileCopyActionBuilder::from_input(input);

		// Verify the copy options are correct
		let copy_options = builder.input.to_copy_options();
		assert!(!copy_options.overwrite);
		assert!(copy_options.verify_checksum);
		assert!(!copy_options.preserve_timestamps);
		assert!(copy_options.delete_after_copy);
	}
}
