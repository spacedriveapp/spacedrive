use crate::JobContext;

use sd_core_job_errors::report::ReportError;
use sd_prisma::prisma::job;
use sd_utils::uuid_to_bytes;

use std::{collections::HashMap, future::Future, pin::Pin, time::Duration};

use serde::{Deserialize, Serialize};

use super::{
	job::{DynJob, OuterContext},
	report::Report,
	JobId, JobSystemError,
};

use sd_core_shared_types::jobs::JobName;

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

// Trait for job serialization handlers
pub trait JobSerializationHandler<OuterCtx: OuterContext, JobCtx: JobContext<OuterCtx>>:
	Send + Sync
{
	fn deserialize_job<'a>(
		&'a self,
		stored_job: StoredJob,
		report: Report,
		ctx: &'a OuterCtx,
	) -> Pin<
		Box<
			dyn Future<
					Output = Result<
						Option<(Box<dyn DynJob<OuterCtx, JobCtx>>, Option<SerializedTasks>)>,
						JobSystemError,
					>,
				> + Send
				+ 'a,
		>,
	>;
}

/// Trait for registering job handlers with the serialization registry.
/// Implement this trait for job types to provide their serialization handlers.
pub trait RegisterJobHandler {
	fn register_handler<OuterCtx: OuterContext, JobCtx: JobContext<OuterCtx>>(
		registry: &mut JobSerializationRegistry<OuterCtx, JobCtx>,
	);
}

// Registry for job serialization handlers
pub struct JobSerializationRegistry<OuterCtx: OuterContext, JobCtx: JobContext<OuterCtx>> {
	handlers: HashMap<JobName, Box<dyn JobSerializationHandler<OuterCtx, JobCtx>>>,
}

impl<OuterCtx: OuterContext, JobCtx: JobContext<OuterCtx>>
	JobSerializationRegistry<OuterCtx, JobCtx>
{
	pub fn new() -> Self {
		Self {
			handlers: HashMap::new(),
		}
	}

	pub fn register_handler(
		&mut self,
		job_name: JobName,
		handler: Box<dyn JobSerializationHandler<OuterCtx, JobCtx>>,
	) {
		self.handlers.insert(job_name, handler);
	}

	pub fn get_handler(
		&self,
		job_name: &JobName,
	) -> Option<&dyn JobSerializationHandler<OuterCtx, JobCtx>> {
		self.handlers.get(job_name).map(|h| h.as_ref())
	}
}

#[macro_export]
macro_rules! impl_job_serialization_handler {
	($handler:ty, $job:ty) => {
		impl<OuterCtx: $crate::job::OuterContext, JobCtx: $crate::job::JobContext<OuterCtx>>
			$crate::store::JobSerializationHandler<OuterCtx, JobCtx> for $handler
		{
			fn deserialize_job<'a>(
				&'a self,
				stored_job: $crate::store::StoredJob,
				report: $crate::report::Report,
				ctx: &'a OuterCtx,
			) -> std::pin::Pin<
				Box<
					dyn std::future::Future<
							Output = Result<
								Option<(
									Box<dyn $crate::job::DynJob<OuterCtx, JobCtx>>,
									Option<$crate::store::SerializedTasks>,
								)>,
								sd_core_job_errors::system::JobSystemError,
							>,
						> + Send
						+ 'a,
				>,
			> {
				Box::pin(async move {
					let $crate::store::StoredJob {
						id,
						name: _,
						run_time,
						serialized_job,
					} = stored_job;

					<$job as $crate::store::SerializableJob<OuterCtx>>::deserialize(
						&serialized_job,
						ctx,
					)
					.await
					.map(|maybe_job| {
						maybe_job.map(|(job, maybe_tasks)| {
							(
								Box::new($crate::job::JobHolder {
									id,
									job,
									run_time,
									report,
									next_jobs: std::collections::VecDeque::new(),
									_ctx: std::marker::PhantomData,
								}) as Box<dyn $crate::job::DynJob<OuterCtx, JobCtx>>,
								maybe_tasks
									.and_then(|tasks| (!tasks.0.is_empty()).then_some(tasks)),
							)
						})
					})
					.map_err(Into::into)
				})
			}
		}
	};
}

pub(super) async fn load_jobs<OuterCtx: OuterContext, JobCtx: JobContext<OuterCtx>>(
	entries: Vec<StoredJobEntry>,
	ctx: &OuterCtx,
	registry: &JobSerializationRegistry<OuterCtx, JobCtx>,
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

		let loaded_job = load_job(root_job, report, ctx, registry).await?;

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
					load_job(next_job, next_report, ctx, registry).await?
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

pub async fn load_job<OuterCtx: OuterContext, JobCtx: JobContext<OuterCtx>>(
	stored_job: StoredJob,
	report: Report,
	ctx: &OuterCtx,
	registry: &JobSerializationRegistry<OuterCtx, JobCtx>,
) -> Result<Option<(Box<dyn DynJob<OuterCtx, JobCtx>>, Option<SerializedTasks>)>, JobSystemError> {
	if let Some(handler) = registry.get_handler(&stored_job.name) {
		handler.deserialize_job(stored_job, report, ctx).await
	} else {
		Ok(None)
	}
}
