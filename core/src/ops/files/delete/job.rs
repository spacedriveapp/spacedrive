//! Delete job implementation

use crate::{domain::addressing::SdPathBatch, infra::job::prelude::*};
use serde::{Deserialize, Serialize};
use std::{
	path::PathBuf,
	time::{Duration, Instant},
};
use tokio::fs;

use super::routing::DeleteStrategyRouter;

/// Delete operation modes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeleteMode {
	/// Move to trash/recycle bin
	Trash,
	/// Permanent deletion (cannot be undone)
	Permanent,
	/// Secure deletion with configurable options.
	/// Uses encryption-aware deletion strategy based on volume encryption status.
	Secure(SecureDeleteOptions),
}

/// Options for secure file deletion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecureDeleteOptions {
	/// Number of overwrite passes. If None, auto-determine based on volume encryption.
	/// - Encrypted volumes (FileVault, BitLocker, LUKS): 1 pass (NIST SP 800-88 guidance)
	/// - Unencrypted SSDs: 1 pass (TRIM is more effective than overwriting)
	/// - Unencrypted HDDs: 3 passes (DOD standard for magnetic media)
	pub passes: Option<u32>,

	/// Whether to use TRIM/hole punching for SSDs instead of overwrite.
	/// This is more effective on SSDs due to wear-leveling.
	/// Default: true (auto-detect SSD and use TRIM if available)
	pub use_trim: bool,

	/// Force overwrite even on encrypted volumes.
	/// By default, encrypted volumes skip multi-pass overwrite since data is ciphertext.
	/// Set to true for maximum paranoia.
	pub force_overwrite: bool,

	/// Truncate file to zero length after erasure (recommended).
	/// This ensures the file metadata is also cleaned up.
	pub truncate_after: bool,
}

impl Default for SecureDeleteOptions {
	fn default() -> Self {
		Self {
			passes: None,      // Auto-determine based on volume
			use_trim: true,    // Use TRIM on SSDs when available
			force_overwrite: false, // Trust encryption
			truncate_after: true,   // Clean up file metadata
		}
	}
}

impl SecureDeleteOptions {
	/// Create options for a quick secure delete (1 pass, trust encryption)
	pub fn quick() -> Self {
		Self {
			passes: Some(1),
			use_trim: true,
			force_overwrite: false,
			truncate_after: true,
		}
	}

	/// Create options for thorough secure delete (3 passes, force overwrite)
	pub fn thorough() -> Self {
		Self {
			passes: Some(3),
			use_trim: false,
			force_overwrite: true,
			truncate_after: true,
		}
	}

	/// Create options for paranoid secure delete (7 passes DOD 5220.22-M)
	pub fn paranoid() -> Self {
		Self {
			passes: Some(7),
			use_trim: false,
			force_overwrite: true,
			truncate_after: true,
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
#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteJob {
	pub targets: SdPathBatch,
	pub mode: DeleteMode,
	pub confirm_permanent: bool,

	// Internal state for resumption
	#[serde(skip)]
	completed_deletions: Vec<usize>,
	#[serde(skip, default = "Instant::now")]
	started_at: Instant,
}

/// Delete progress information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteProgress {
	pub current_file: String,
	pub files_deleted: usize,
	pub total_files: usize,
	pub bytes_deleted: u64,
	pub total_bytes: u64,
	pub current_operation: String,
	pub estimated_remaining: Option<Duration>,
}

impl JobProgress for DeleteProgress {}

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
		ctx.log(format!(
			"Starting {} deletion of {} files",
			match &self.mode {
				DeleteMode::Trash => "trash".to_string(),
				DeleteMode::Permanent => "permanent".to_string(),
				DeleteMode::Secure(opts) => format!(
					"secure (passes: {}, trim: {}, force: {})",
					opts.passes.map_or("auto".to_string(), |p| p.to_string()),
					opts.use_trim,
					opts.force_overwrite
				),
			},
			self.targets.paths.len()
		));

		// Safety check for permanent deletion
		if matches!(self.mode, DeleteMode::Permanent | DeleteMode::Secure(_))
			&& !self.confirm_permanent
		{
			return Err(JobError::execution(
				"Permanent deletion requires explicit confirmation",
			));
		}

		// Validate targets exist (only for local paths)
		self.validate_targets(&ctx).await?;

		// Select strategy based on path topology
		let volume_manager = ctx.volume_manager();
		let strategy =
			DeleteStrategyRouter::select_strategy(&self.targets.paths, volume_manager.as_deref())
				.await;

		let strategy_description =
			DeleteStrategyRouter::describe_strategy(&self.targets.paths).await;
		ctx.log(format!("Using strategy: {}", strategy_description));

		// Execute deletion using selected strategy
		let results = strategy
			.execute(&ctx, &self.targets.paths, self.mode.clone())
			.await
			.map_err(|e| JobError::execution(format!("Strategy execution failed: {}", e)))?;

		// Aggregate results
		let deleted_count = results.iter().filter(|r| r.success).count();
		let failed_count = results.len() - deleted_count;
		let total_bytes: u64 = results.iter().map(|r| r.bytes_freed).sum();

		let failed_deletions = results
			.into_iter()
			.filter(|r| !r.success)
			.map(|r| DeleteError {
				path: r
					.path
					.as_local_path()
					.map(|p| p.to_path_buf())
					.unwrap_or_default(),
				error: r.error.unwrap_or_default(),
			})
			.collect();

		ctx.log(format!(
			"Delete operation completed: {} deleted, {} failed",
			deleted_count, failed_count
		));

		Ok(DeleteOutput {
			deleted_count,
			failed_count,
			total_bytes,
			duration: self.started_at.elapsed(),
			failed_deletions,
			mode: self.mode.clone(),
		})
	}
}

impl DeleteJob {
	/// Create a new delete job
	pub fn new(targets: SdPathBatch, mode: DeleteMode) -> Self {
		Self {
			targets,
			mode,
			confirm_permanent: false,
			completed_deletions: Vec::new(),
			started_at: Instant::now(),
		}
	}

	/// Create a trash operation
	pub fn trash(targets: SdPathBatch) -> Self {
		Self::new(targets, DeleteMode::Trash)
	}

	/// Create a permanent delete operation (requires confirmation)
	pub fn permanent(targets: SdPathBatch, confirmed: bool) -> Self {
		let mut job = Self::new(targets, DeleteMode::Permanent);
		job.confirm_permanent = confirmed;
		job
	}

	/// Create a secure delete operation with default options (requires confirmation)
	pub fn secure(targets: SdPathBatch, confirmed: bool) -> Self {
		Self::secure_with_options(targets, confirmed, SecureDeleteOptions::default())
	}

	/// Create a secure delete operation with custom options (requires confirmation)
	pub fn secure_with_options(
		targets: SdPathBatch,
		confirmed: bool,
		options: SecureDeleteOptions,
	) -> Self {
		let mut job = Self::new(targets, DeleteMode::Secure(options));
		job.confirm_permanent = confirmed;
		job
	}

	/// Validate that all targets exist (only for local paths)
	async fn validate_targets(&self, _ctx: &JobContext<'_>) -> JobResult<()> {
		for target in &self.targets.paths {
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
}

/// Error information for failed deletions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteError {
	pub path: PathBuf,
	pub error: String,
}

/// Job output for delete operations
#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteOutput {
	pub deleted_count: usize,
	pub failed_count: usize,
	pub total_bytes: u64,
	pub duration: Duration,
	pub failed_deletions: Vec<DeleteError>,
	pub mode: DeleteMode,
}

impl From<DeleteOutput> for JobOutput {
	fn from(output: DeleteOutput) -> Self {
		JobOutput::FileDelete {
			deleted_count: output.deleted_count,
			failed_count: output.failed_count,
			total_bytes: output.total_bytes,
		}
	}
}
