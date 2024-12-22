use async_trait::async_trait;

use super::{
	job::{DynJob, Job, JobContext, JobTaskDispatcher, OuterContext},
	report::Report,
	JobId, JobSystem, JobSystemError,
};

use sd_core_shared_errors::job::Error;

/// Extension trait for jobs that can be undone
#[async_trait]
pub trait ReversibleJob: Job {
	/// Check if this job can be undone in its current state
	/// This will query the database to check if the job exists and has valid state
	async fn can_undo<OuterCtx: OuterContext>(
		job_id: JobId,
		ctx: &impl JobContext<OuterCtx>,
	) -> Result<bool, Error> {
		// Query the job from the database to check if it can be undone
		let job = ctx
			.db()
			.job()
			.find_unique(sd_prisma::prisma::job::id::equals(sd_utils::uuid_to_bytes(
				&job_id,
			)))
			.exec()
			.await
			.map_err(JobSystemError::DatabaseError)?
			.ok_or_else(|| JobSystemError::NotFound(job_id))?;

		// Convert job data to Report for easier access to metadata
		let report = Report::try_from(job).map_err(|e| JobSystemError::from(e))?;

		// Job specific logic to determine if it can be undone based on its state
		Self::validate_undo_state(&report).await
	}

	/// Validate if the job's stored state allows for an undo operation
	/// This is job-specific logic that examines the job's state in the database
	async fn validate_undo_state(report: &Report) -> Result<bool, Error>;

	/// Create a new job that will undo the operations of the original job
	/// This will be called to create the job that will be dispatched
	async fn create_reverse_job<OuterCtx: OuterContext, JobCtx: JobContext<OuterCtx>>(
		report: &Report,
		ctx: &JobCtx,
	) -> Result<Box<dyn DynJob<OuterCtx, JobCtx>>, Error>;

	/// Undo the job's operations by creating and dispatching a new job
	/// This will be called when the job needs to be reversed
	async fn undo<OuterCtx: OuterContext, JobCtx: JobContext<OuterCtx>>(
		job_id: JobId,
		dispatcher: &JobTaskDispatcher,
		ctx: &JobCtx,
		job_system: &JobSystem<OuterCtx, JobCtx>,
	) -> Result<JobId, Error> {
		// Get job from database and convert to Report
		let job = ctx
			.db()
			.job()
			.find_unique(sd_prisma::prisma::job::id::equals(sd_utils::uuid_to_bytes(
				&job_id,
			)))
			.exec()
			.await
			.map_err(JobSystemError::DatabaseError)?
			.ok_or_else(|| JobSystemError::NotFound(job_id))?;

		let report = Report::try_from(job).map_err(|e| JobSystemError::from(e))?;

		// Create the reverse job
		let dyn_job = Self::create_reverse_job(&report, ctx).await?;

		// Dispatch the job through the JobSystem
		// let reverse_job_id = job_system
		// 	.dispatch(dyn_job, ctx.get_outer_ctx())
		// 	.await
		// 	.map_err(Error::from)?;

		// Ok(reverse_job_id)
		unimplemented!();
	}
}
