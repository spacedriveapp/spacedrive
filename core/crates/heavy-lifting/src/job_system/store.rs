use crate::{file_identifier, indexer, media_processor, JobContext};

use sd_prisma::prisma::job;
use sd_utils::uuid_to_bytes;

use std::{
	collections::{HashMap, VecDeque},
	future::Future,
	marker::PhantomData,
	time::Duration,
};

use serde::{Deserialize, Serialize};

use super::{
	job::{DynJob, Job, JobHolder, JobName, OuterContext},
	report::{Report, ReportError},
	JobId, JobSystemError,
};

#[derive(Debug, Serialize, Deserialize)]
#[repr(transparent)]
#[serde(transparent)]
pub struct SerializedTasks(pub Vec<u8>);

pub trait SerializableJob<OuterCtx: OuterContext>: 'static
where
	Self: Sized,
{
	fn serialize(
		self,
	) -> impl Future<Output = Result<Option<Vec<u8>>, rmp_serde::encode::Error>> + Send {
		async move { Ok(None) }
	}

	#[allow(unused_variables)]
	fn deserialize(
		serialized_job: &[u8],
		ctx: &OuterCtx,
	) -> impl Future<
		Output = Result<Option<(Self, Option<SerializedTasks>)>, rmp_serde::decode::Error>,
	> + Send {
		async move { Ok(None) }
	}
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StoredJob {
	pub(super) id: JobId,
	pub(super) name: JobName,
	pub(super) run_time: Duration,
	pub(super) serialized_job: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StoredJobEntry {
	pub(super) root_job: StoredJob,
	pub(super) next_jobs: Vec<StoredJob>,
}

pub(super) async fn load_jobs<OuterCtx: OuterContext, JobCtx: JobContext<OuterCtx>>(
	entries: Vec<StoredJobEntry>,
	ctx: &OuterCtx,
) -> Result<Vec<(Box<dyn DynJob<OuterCtx, JobCtx>>, Option<SerializedTasks>)>, JobSystemError> {
	let mut jobs_to_load = Vec::with_capacity(entries.len());
	let mut job_ids = Vec::new();

	// First collect all job IDs we need to load reports for
	for StoredJobEntry {
		root_job,
		next_jobs,
	} in &entries
	{
		job_ids.push(uuid_to_bytes(&root_job.id));
		job_ids.extend(next_jobs.iter().map(|job| uuid_to_bytes(&job.id)));
	}

	// Load all reports at once
	let mut reports: HashMap<JobId, Report> = ctx
		.db()
		.job()
		.find_many(vec![job::id::in_vec(job_ids)])
		.take(100)
		.exec()
		.await?
		.into_iter()
		.map(|data| Report::try_from(data).map(|report| (report.id, report)))
		.collect::<Result<_, ReportError>>()?;

	// Process each entry
	for StoredJobEntry {
		root_job,
		next_jobs,
	} in entries
	{
		let report = reports
			.remove(&root_job.id)
			.ok_or(ReportError::MissingReport(root_job.id))?;

		let loaded_job = load_job(root_job, report, ctx).await?;

		if let Some((mut dyn_job, tasks)) = loaded_job {
			// Load and set next jobs
			let next_jobs_loaded = next_jobs
				.into_iter()
				.map(|next_job| {
					let next_job_id = next_job.id;
					reports
						.remove(&next_job.id)
						.map(|report| (next_job, report))
						.ok_or(ReportError::MissingReport(next_job_id))
				})
				.collect::<Result<Vec<_>, _>>()?;

			let mut next_dyn_jobs = Vec::new();
			for (next_job, next_report) in next_jobs_loaded {
				if let Some((next_dyn_job, next_tasks)) =
					load_job(next_job, next_report, ctx).await?
				{
					assert!(
						next_tasks.is_none(),
						"Next jobs must not have tasks as they haven't run yet"
					);
					assert!(
						next_dyn_job.next_jobs().is_empty(),
						"Next jobs must not have next jobs"
					);
					next_dyn_jobs.push(next_dyn_job);
				}
			}

			dyn_job.set_next_jobs(next_dyn_jobs.into());
			jobs_to_load.push((dyn_job, tasks));
		}
	}

	Ok(jobs_to_load)
}

macro_rules! match_deserialize_job {
	($stored_job:ident, $report:ident, $outer_ctx:ident, $outer_ctx_type:ty, $job_ctx_type:ty, [$($job_type:ty),+ $(,)?]) => {{
		let StoredJob {
			id,
			name,
			run_time,
			serialized_job,
		} = $stored_job;


		match name {
			$(<$job_type as Job>::NAME => <$job_type as SerializableJob<$outer_ctx_type>>::deserialize(
					&serialized_job,
					$outer_ctx,
				).await
					.map(|maybe_job| maybe_job.map(|(job, maybe_tasks)| -> (
							Box<dyn DynJob<$outer_ctx_type, $job_ctx_type>>,
							Option<SerializedTasks>
						) {
							(
								Box::new(JobHolder {
									id,
									job,
									run_time,
									report: $report,
									next_jobs: VecDeque::new(),
									_ctx: PhantomData,
								}),
								maybe_tasks.and_then(
									|tasks| (!tasks.0.is_empty()).then_some(tasks)
								),
							)
						}
					))
					.map_err(Into::into),)+

			// TODO(fogodev): this is temporary until we can get rid of the old job system
			_ => unimplemented!("Job not implemented"),
		}
	}};
}

async fn load_job<OuterCtx: OuterContext, JobCtx: JobContext<OuterCtx>>(
	stored_job: StoredJob,
	report: Report,
	ctx: &OuterCtx,
) -> Result<Option<(Box<dyn DynJob<OuterCtx, JobCtx>>, Option<SerializedTasks>)>, JobSystemError> {
	match_deserialize_job!(
		stored_job,
		report,
		ctx,
		OuterCtx,
		JobCtx,
		[
			indexer::job::Indexer,
			file_identifier::job::FileIdentifier,
			media_processor::job::MediaProcessor,
			// TODO: Add more jobs here
		]
	)
}
