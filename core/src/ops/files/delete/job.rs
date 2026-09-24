//! Delete job implementation

use crate::{
	domain::addressing::SdPath,
	infra::job::{generic_progress::GenericProgress, prelude::*},
};
use serde::{Deserialize, Serialize};
use std::{
	path::PathBuf,
	time::{Duration, Instant},
};
use tokio::fs;

use super::compared;
use super::duplicates;
use super::input::DeleteTargets;
use super::routing::DeleteStrategyRouter;
use super::strategy::DeleteResult;

/// Delete operation modes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeleteMode {
	/// Move to trash/recycle bin
	Trash,
	/// Permanent deletion (cannot be undone)
	Permanent,
	/// Secure deletion (overwrite data)
	Secure,
}

impl DeleteMode {
	pub(super) fn label(&self) -> &'static str {
		match self {
			Self::Trash => "trash",
			Self::Permanent => "permanent",
			Self::Secure => "secure",
		}
	}
}

/// Options for file delete operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteOptions {
	pub permanent: bool,
	pub recursive: bool,
}

impl Default for DeleteOptions {
	fn default() -> Self {
		Self {
			permanent: false,
			recursive: false,
		}
	}
}

/// Delete job for removing files and directories
#[derive(Debug, Serialize, Deserialize, Job)]
pub struct DeleteJob {
	pub targets: DeleteTargets,
	pub mode: DeleteMode,

	#[serde(skip, default = "Instant::now")]
	started_at: Instant,
}

impl Job for DeleteJob {
	const NAME: &'static str = "delete_files";
	const RESUMABLE: bool = true;
	const DESCRIPTION: Option<&'static str> = Some("Delete files and directories");
}

impl crate::infra::job::traits::DynJob for DeleteJob {
	fn job_name(&self) -> &'static str {
		Self::NAME
	}

	// DeleteJob doesn't track specific entry resources, so use default None
}

#[async_trait::async_trait]
impl JobHandler for DeleteJob {
	type Output = DeleteOutput;

	async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
		match &self.targets {
			DeleteTargets::Paths { paths } => {
				delete_paths(&ctx, paths.clone(), self.mode.clone(), self.started_at).await
			}
			DeleteTargets::Comparison { comparison } => {
				compared::delete(&ctx, comparison, self.mode.clone(), self.started_at).await
			}
			DeleteTargets::Duplicates { duplicates } => {
				duplicates::delete(&ctx, duplicates, self.mode.clone(), self.started_at).await
			}
		}
	}
}

impl DeleteJob {
	/// Create a new delete job
	pub fn new(targets: DeleteTargets, mode: DeleteMode) -> Self {
		Self {
			targets,
			mode,
			started_at: Instant::now(),
		}
	}
}

/// Delete files named one by one.
async fn delete_paths(
	ctx: &JobContext<'_>,
	paths: Vec<SdPath>,
	mode: DeleteMode,
	started_at: Instant,
) -> JobResult<DeleteOutput> {
	let total_files = paths.len();
	ctx.log(format!(
		"Starting {} deletion of {} files",
		mode.label(),
		total_files
	));

	// Phase: Preparing
	ctx.progress(Progress::Indeterminate(format!(
		"Validating {} targets",
		total_files
	)));
	validate_targets(&paths).await?;

	// Phase: Resolving paths
	ctx.progress(Progress::Indeterminate("Resolving paths".to_string()));

	// Resolve Content paths to Physical paths before strategy selection
	let mut resolved = Vec::with_capacity(paths.len());
	for path in &paths {
		resolved.push(
			path.resolve_in_job(ctx)
				.await
				.map_err(|e| JobError::execution(format!("Failed to resolve path: {e}")))?,
		);
	}

	// Select strategy based on path topology
	let volume_manager = ctx.volume_manager();
	let strategy =
		DeleteStrategyRouter::select_strategy(&resolved, volume_manager.as_deref()).await;
	ctx.log(format!(
		"Using strategy: {}",
		DeleteStrategyRouter::describe_strategy(&resolved).await
	));

	// Phase: Deleting
	ctx.progress(Progress::Indeterminate(format!(
		"Deleting {} files ({})",
		total_files,
		mode.label()
	)));

	let results = strategy
		.execute(ctx, &resolved, mode.clone())
		.await
		.map_err(|e| JobError::execution(format!("Strategy execution failed: {}", e)))?;

	let mut tally = Tally::default();
	tally.record(results);

	// Phase: Complete
	ctx.progress(Progress::Generic(
		GenericProgress::new(
			1.0,
			"Complete",
			format!("{} deleted, {} failed", tally.deleted, tally.failed.len()),
		)
		.with_completion(total_files as u64, total_files as u64)
		.with_bytes(tally.bytes, tally.bytes)
		.with_performance(0.0, None, Some(started_at.elapsed()))
		.with_errors(tally.failed.len() as u64, 0),
	));

	ctx.log(format!(
		"Delete operation completed: {} deleted, {} failed",
		tally.deleted,
		tally.failed.len()
	));

	Ok(tally.into_output(mode, started_at))
}

/// Validate that all targets exist (only for local paths)
async fn validate_targets(targets: &[SdPath]) -> JobResult<()> {
	for target in targets {
		if let Some(local_path) = target.as_local_path() {
			if !fs::try_exists(local_path).await.unwrap_or(false) {
				return Err(JobError::execution(format!(
					"Target does not exist: {}",
					local_path.display()
				)));
			}
		}
	}
	Ok(())
}

/// What a deletion has done so far.
#[derive(Default)]
pub(super) struct Tally {
	pub(super) deleted: usize,
	pub(super) bytes: u64,
	pub(super) failed: Vec<DeleteError>,
	pub(super) skipped: Vec<DeleteSkip>,
}

impl Tally {
	pub(super) fn record(&mut self, results: Vec<DeleteResult>) {
		for result in results {
			if result.success {
				self.deleted += 1;
				self.bytes += result.bytes_freed;
			} else {
				self.failed.push(DeleteError {
					path: result
						.path
						.as_local_path()
						.map(|p| p.to_path_buf())
						.unwrap_or_default(),
					error: result.error.unwrap_or_default(),
				});
			}
		}
	}

	pub(super) fn skip(&mut self, path: PathBuf, reason: SkipReason) {
		self.skipped.push(DeleteSkip { path, reason });
	}

	pub(super) fn into_output(self, mode: DeleteMode, started_at: Instant) -> DeleteOutput {
		DeleteOutput {
			deleted_count: self.deleted,
			failed_count: self.failed.len(),
			skipped_count: self.skipped.len(),
			total_bytes: self.bytes,
			duration: started_at.elapsed(),
			failed_deletions: self.failed,
			skipped: self.skipped,
			mode,
		}
	}
}

/// Error information for failed deletions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteError {
	pub path: PathBuf,
	pub error: String,
}

/// A file a comparison or a set of duplicates named that the job left in
/// place.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteSkip {
	pub path: PathBuf,
	pub reason: SkipReason,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SkipReason {
	/// Read in full, the file and the copy that was to stand in for it hold
	/// different bytes.
	Differs,
	/// No copy was there to read by the time the file's turn came.
	NoCopy,
	/// The file or its copy could not be read.
	Unreadable(String),
	/// The index holds no content hash for the file, so its copies cannot
	/// be found.
	Unhashed,
}

impl std::fmt::Display for SkipReason {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Differs => write!(f, "the copy that stays holds different bytes"),
			Self::NoCopy => write!(f, "the copy that was to stay is gone"),
			Self::Unreadable(error) => write!(f, "could not be read: {error}"),
			Self::Unhashed => write!(f, "has not been hashed yet"),
		}
	}
}

/// Job output for delete operations
#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteOutput {
	pub deleted_count: usize,
	pub failed_count: usize,
	pub skipped_count: usize,
	pub total_bytes: u64,
	pub duration: Duration,
	pub failed_deletions: Vec<DeleteError>,
	pub skipped: Vec<DeleteSkip>,
	pub mode: DeleteMode,
}

impl From<DeleteOutput> for JobOutput {
	fn from(output: DeleteOutput) -> Self {
		JobOutput::FileDelete {
			deleted_count: output.deleted_count,
			failed_count: output.failed_count,
			skipped_count: output.skipped_count,
			total_bytes: output.total_bytes,
		}
	}
}
