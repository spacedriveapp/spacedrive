use crate::{
	jobs::{indexer::IndexerJob, JobId},
	Error,
};

use sd_prisma::prisma::{job, PrismaClient};
use sd_task_system::Task;
use sd_utils::uuid_to_bytes;

use std::{
	collections::{HashMap, VecDeque},
	iter,
};

use serde::{Deserialize, Serialize};

use super::{
	job::{DynJob, JobHolder, JobName},
	report::{Report, ReportError},
	JobSystemError,
};

type DynTasks = Vec<Box<dyn Task<Error>>>;

pub trait SerializableJob: 'static
where
	Self: Sized,
{
	fn serialize(&self) -> Option<Result<Vec<u8>, rmp_serde::encode::Error>>;
	fn deserialize(serialized_job: Vec<u8>) -> Result<(Self, DynTasks), rmp_serde::decode::Error>;
}

pub type DynJobAndTasks = (Box<dyn DynJob>, DynTasks);

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

pub async fn load_jobs(
	entries: Vec<StoredJobEntry>,
	db: &PrismaClient,
) -> Result<Vec<DynJobAndTasks>, JobSystemError> {
	let mut reports = db
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
				let (mut dyn_job, tasks) = load_job(root_job, report)?;

				dyn_job.set_next_jobs(
					next_jobs
						.into_iter()
						.map(|next_job| {
							let next_job_report = reports
								.remove(&next_job.id)
								.ok_or(ReportError::MissingReport(next_job.id))?;

							let (next_dyn_job, next_tasks) = load_job(next_job, next_job_report)?;

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
	($stored_job:ident, $report:ident, [$(($job_name:pat, $job:ty)),+ $(,)?]) => {{
		let StoredJob {
			id,
			name,
			serialized_job,
		} = $stored_job;

		let report: Report = $report;

		match name {
			$($job_name => <$job as SerializableJob>::deserialize(serialized_job)
				.map(|(job, tasks)| -> DynJobAndTasks {
					(
						Box::new(JobHolder {
							id,
							job,
							report,
							next_jobs: VecDeque::new(),
						}),
						tasks,
					)
				})
				.map_err(Into::into),)+
		}
	}};
}

fn load_job(stored_job: StoredJob, report: Report) -> Result<DynJobAndTasks, JobSystemError> {
	match_deserialize_job!(
		stored_job,
		report,
		[
			(JobName::Indexer, IndexerJob),
			// TODO: Add more jobs here
			// e.g.: (JobName::FileIdentifier, FileIdentifierJob),
		]
	)
}
