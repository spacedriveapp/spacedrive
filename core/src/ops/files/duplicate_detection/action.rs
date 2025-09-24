//! File duplicate detection action handler

use super::input::DuplicateDetectionInput;
use super::job::{DetectionMode, DuplicateDetectionJob};
use crate::{
	context::CoreContext,
	domain::addressing::{SdPath, SdPathBatch},
	infra::{
		action::{error::ActionError, LibraryAction},
		job::handle::JobHandle,
	},
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DuplicateDetectionAction {
	pub paths: SdPathBatch,
	pub algorithm: String,
	pub threshold: f64,
}

impl DuplicateDetectionAction {
	/// Create a new duplicate detection action
	pub fn new(paths: SdPathBatch, algorithm: String, threshold: f64) -> Self {
		Self {
			paths,
			algorithm,
			threshold,
		}
	}
}

pub struct DuplicateDetectionHandler;

impl DuplicateDetectionHandler {
	pub fn new() -> Self {
		Self
	}
}

// Implement the unified LibraryAction (replaces ActionHandler)
impl LibraryAction for DuplicateDetectionAction {
	type Input = DuplicateDetectionInput;
	type Output = crate::infra::job::handle::JobReceipt;

	fn from_input(i: Self::Input) -> Result<Self, String> {
		let sd_paths = i
			.paths
			.into_iter()
			.map(|p| SdPath::local(p))
			.collect::<Vec<_>>();
		Ok(DuplicateDetectionAction {
			paths: SdPathBatch { paths: sd_paths },
			algorithm: i.algorithm,
			threshold: i.threshold,
		})
	}

	async fn execute(
		self,
		library: std::sync::Arc<crate::library::Library>,
		context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		// Library is pre-validated by ActionManager

		// Create duplicate detection job
		let mode = match self.algorithm.as_str() {
			"size_only" => DetectionMode::SizeOnly,
			"name_and_size" => DetectionMode::NameAndSize,
			"deep_scan" => DetectionMode::DeepScan,
			_ => DetectionMode::ContentHash,
		};

		let job = DuplicateDetectionJob::new(self.paths, mode);

		// Dispatch job and return handle directly
		let job_handle = library
			.jobs()
			.dispatch(job)
			.await
			.map_err(ActionError::Job)?;

		Ok(job_handle.into())
	}

	fn action_kind(&self) -> &'static str {
		"file.detect_duplicates"
	}

	async fn validate(
		&self,
		_library: std::sync::Arc<crate::library::Library>,
		_context: std::sync::Arc<crate::context::CoreContext>,
	) -> Result<(), ActionError> {
		if self.paths.paths.is_empty() {
			return Err(ActionError::Validation {
				field: "paths".to_string(),
				message: "At least one path must be specified".to_string(),
			});
		}
		Ok(())
	}
}

// Register with the registry
crate::register_library_action!(DuplicateDetectionAction, "files.duplicate_detection");
