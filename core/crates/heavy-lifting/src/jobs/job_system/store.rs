use crate::{
	jobs::{indexer::IndexerJob, JobId},
	Error,
};

use sd_prisma::prisma::job;
use sd_task_system::Task;
use sd_utils::uuid_to_bytes;

use std::{
	collections::{HashMap, VecDeque},
	iter,
	marker::PhantomData,
};

use serde::{Deserialize, Serialize};

use super::{
	job::{DynJob, Job, JobContext, JobHolder, JobName},
	report::{Report, ReportError},
	JobSystemError,
};

type DynTasks = Vec<Box<dyn Task<Error>>>;

pub trait SerializableJob: 'static
where
	Self: Sized,
{
	fn serialize(&self) -> Option<Result<Vec<u8>, rmp_serde::encode::Error>>;
	fn deserialize(
		serialized_job: Vec<u8>,
		ctx: &impl JobContext,
	) -> Result<(Self, DynTasks), rmp_serde::decode::Error>;
}

pub type DynJobAndTasks<Ctx> = (Box<dyn DynJob<Ctx>>, DynTasks);

#[derive(Debug, Serialize, Deserialize)]
pub struct StoredJob {
	pub(super) id: JobId,
	pub(super) name: JobName,
	pub(super) serialized_job: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StoredJobEntry {
	pub(super) root_job: StoredJob,
	pub(super) next_jobs: Vec<StoredJob>,
}

pub async fn load_jobs<Ctx: JobContext>(
	entries: Vec<StoredJobEntry>,
	job_ctx: &Ctx,
) -> Result<Vec<DynJobAndTasks<Ctx>>, JobSystemError> {
	let mut reports = job_ctx
		.db()
		.job()
		.find_many(vec![job::id::in_vec(
			entries
				.iter()
				.flat_map(
					|StoredJobEntry {
					     root_job: StoredJob { id, .. },
					     next_jobs,
					 }| { iter::once(*id).chain(next_jobs.iter().map(|StoredJob { id, .. }| *id)) },
				)
				.map(uuid_to_bytes)
				.collect::<Vec<_>>(),
		)])
		.exec()
		.await
		.map_err(JobSystemError::LoadReportsForResume)?
		.into_iter()
		.map(Report::try_from)
		.map(|report_res| report_res.map(|report| (report.id, report)))
		.collect::<Result<HashMap<_, _>, _>>()?;

	entries
		.into_iter()
		.map(
			|StoredJobEntry {
			     root_job,
			     next_jobs,
			 }| {
				let report = reports
					.remove(&root_job.id)
					.ok_or(ReportError::MissingReport(root_job.id))?;
				let (mut dyn_job, tasks) = load_job(root_job, report, job_ctx)?;

				dyn_job.set_next_jobs(
					next_jobs
						.into_iter()
						.map(|next_job| {
							let next_job_report = reports
								.remove(&next_job.id)
								.ok_or(ReportError::MissingReport(next_job.id))?;

							let (next_dyn_job, next_tasks) =
								load_job(next_job, next_job_report, job_ctx)?;

							assert!(next_tasks.is_empty(), "Next jobs must not have tasks");
							assert!(
								next_dyn_job.next_jobs().is_empty(),
								"Next jobs must not have next jobs"
							);

							Ok(next_dyn_job)
						})
						.collect::<Result<VecDeque<_>, JobSystemError>>()?,
				);

				Ok((dyn_job, tasks))
			},
		)
		.collect::<Result<Vec<_>, _>>()
}

macro_rules! match_deserialize_job {
	($stored_job:ident, $report:ident, $job_ctx:ident, $ctx_type:ty, [$($job_type:ty),+ $(,)?]) => {{
		let StoredJob {
			id,
			name,
			serialized_job,
		} = $stored_job;

		match name {
			$(<$job_type as Job<$ctx_type>>::NAME => <$job_type as SerializableJob>::deserialize(
					serialized_job,
					$job_ctx
				)
					.map(|(job, tasks)| -> DynJobAndTasks<$ctx_type> {
						(
							Box::new(JobHolder {
								id,
								job,
								report: $report,
								next_jobs: VecDeque::new(),
								_ctx: PhantomData,
							}),
							tasks,
						)
					})
					.map_err(Into::into),)+
		}
	}};
}

fn load_job<Ctx: JobContext>(
	stored_job: StoredJob,
	report: Report,
	job_ctx: &Ctx,
) -> Result<DynJobAndTasks<Ctx>, JobSystemError> {
	match_deserialize_job!(
		stored_job,
		report,
		job_ctx,
		Ctx,
		[
			IndexerJob,
			// TODO: Add more jobs here
			// e.g.: FileIdentifierJob, MediaProcessorJob, etc.,
		]
	)
}
