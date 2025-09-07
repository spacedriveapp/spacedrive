//! Core traits for defining jobs

use super::{
	context::JobContext,
	error::JobResult,
	output::JobOutput,
	types::{ErasedJob, JobSchema},
};
use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};
use std::any::Any;

/// Main trait for defining a job
pub trait Job: Serialize + DeserializeOwned + Send + Sync + 'static {
	/// Job name - must be unique
	const NAME: &'static str;

	/// Whether this job can be resumed after interruption
	const RESUMABLE: bool = true;

	/// Schema version for migrations
	const VERSION: u32 = 1;

	/// Optional description
	const DESCRIPTION: Option<&'static str> = None;

	/// Get the job schema
	fn schema() -> JobSchema {
		JobSchema {
			name: Self::NAME,
			resumable: Self::RESUMABLE,
			version: Self::VERSION,
			description: Self::DESCRIPTION,
		}
	}
}

/// Handler trait that defines job execution logic
#[async_trait]
pub trait JobHandler: Job {
	/// Output type for this job
	type Output: Into<JobOutput> + Send;

	/// Run the job
	async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output>;

	/// Called when job is paused (optional)
	async fn on_pause(&mut self, _ctx: &JobContext<'_>) -> JobResult {
		Ok(())
	}

	/// Called when job is resumed (optional)
	async fn on_resume(&mut self, _ctx: &JobContext<'_>) -> JobResult {
		Ok(())
	}

	/// Called when job is cancelled (optional)
	async fn on_cancel(&mut self, _ctx: &JobContext<'_>) -> JobResult {
		Ok(())
	}
}

/// Trait for jobs that can be serialized
pub trait SerializableJob: Job {
	/// Serialize job state
	fn serialize_state(&self) -> JobResult<Vec<u8>> {
		rmp_serde::to_vec(self).map_err(|e| super::error::JobError::serialization(format!("{}", e)))
	}

	/// Deserialize job state
	fn deserialize_state(data: &[u8]) -> JobResult<Self> {
		rmp_serde::from_slice(data)
			.map_err(|e| super::error::JobError::serialization(format!("{}", e)))
	}
}

// Blanket implementation for all Jobs
impl<T: Job> SerializableJob for T {}

/// Progress reporter trait for jobs with custom progress
pub trait ProgressReporter {
	/// Progress type for this job
	type Progress: super::progress::JobProgress;
}

/// Resource requirements for a job
pub trait ResourceRequirements {
	/// Maximum number of concurrent instances
	fn max_concurrent() -> Option<usize> {
		None
	}

	/// Required resources
	fn required_resources() -> Vec<ResourceRequirement> {
		vec![]
	}
}

/// A required resource
#[derive(Debug, Clone)]
pub enum ResourceRequirement {
	/// Named resource (e.g., "gpu")
	Named(&'static str),
	/// Disk space in bytes
	DiskSpace(u64),
	/// Memory in bytes
	Memory(u64),
}

/// Job dependencies
pub trait JobDependencies {
	/// Jobs that must complete before this one
	fn dependencies() -> &'static [&'static str] {
		&[]
	}

	/// Jobs that should run after this one
	fn run_after() -> &'static [&'static str] {
		&[]
	}
}

/// A trait for jobs that operate on specific VDFS resources.
/// This allows the system to query which entries a job is currently affecting.
pub trait Resourceful {
	/// Returns a list of entry IDs that this job is currently processing.
	/// This can be called at any point during the job's lifecycle.
	fn get_affected_resources(&self) -> Vec<i32>;
}

/// A dyn-compatible trait for dynamic job operations
/// This is separate from Job to avoid serialization trait bounds
pub trait DynJob: Send + Sync {
	/// Job name for identification
	fn job_name(&self) -> &'static str;

	/// Try to get affected resources if this job is resourceful
	/// Returns None if the job doesn't track specific resources
	fn try_get_affected_resources(&self) -> Option<Vec<i32>> {
		None // Default implementation for non-resourceful jobs
	}
}

// Jobs that operate on specific entries should implement Resourceful
