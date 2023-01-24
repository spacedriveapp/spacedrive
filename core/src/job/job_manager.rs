use crate::{
	invalidate_query,
	job::{JobError, JobReport, WorkerContext},
	library::LibraryContext,
	location::indexer::indexer_job::IndexerJob,
	object::{
		fs::{
			copy::FileCopierJob, cut::FileCutterJob, decrypt::FileDecryptorJob,
			delete::FileDeleterJob, encrypt::FileEncryptorJob, erase::FileEraserJob,
		},
		identifier_job::full_identifier_job::FullFileIdentifierJob,
		preview::ThumbnailJob,
		validation::validator_job::ObjectValidatorJob,
	},
	prisma::job,
};

use std::{
	collections::{HashMap, VecDeque},
	sync::Arc,
	time::Duration,
};

use int_enum::IntEnum;
use once_cell::sync::Lazy;
use prisma_client_rust::or;
use tokio::{
	sync::{broadcast, oneshot, RwLock},
	time::sleep,
};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use super::{JobRestorer, JobState, JobStatus, StatefulJob};

// db is single threaded, nerd
const MAX_WORKERS: usize = 1;

/// number of job steps before we persist the job state.
/// A persist will:
///   - Update `JobManager.running` with the latest `JobReport`.
///   - Write `JobReport` to the DB
const STEPS_BETWEEN_PERSIST: usize = 10; // TODO: Tune this constant

/// JOB_RESTORER is a map of job names to their restorer traits.
/// This allows us to be sure when dispatching a job that it has the ability to be restored on restart because that was a common mistake with the old system.
const JOB_RESTORER: Lazy<HashMap<&'static str, Box<dyn JobRestorer>>> = Lazy::new(|| {
	HashMap::from([
		(
			<ThumbnailJob as StatefulJob>::NAME,
			Box::new(ThumbnailJob {}) as Box<dyn JobRestorer>,
		),
		(
			<IndexerJob as StatefulJob>::NAME,
			Box::new(IndexerJob {}) as Box<dyn JobRestorer>,
		),
		(
			<FullFileIdentifierJob as StatefulJob>::NAME,
			Box::new(FullFileIdentifierJob {}) as Box<dyn JobRestorer>,
		),
		(
			<ObjectValidatorJob as StatefulJob>::NAME,
			Box::new(ObjectValidatorJob {}) as Box<dyn JobRestorer>,
		),
		(
			<FileCutterJob as StatefulJob>::NAME,
			Box::new(FileCutterJob {}) as Box<dyn JobRestorer>,
		),
		(
			<FileCopierJob as StatefulJob>::NAME,
			Box::new(FileCopierJob {}) as Box<dyn JobRestorer>,
		),
		(
			<FileDeleterJob as StatefulJob>::NAME,
			Box::new(FileDeleterJob {}) as Box<dyn JobRestorer>,
		),
		(
			<FileEraserJob as StatefulJob>::NAME,
			Box::new(FileEraserJob {}) as Box<dyn JobRestorer>,
		),
		(
			<FileEncryptorJob as StatefulJob>::NAME,
			Box::new(FileEncryptorJob {}) as Box<dyn JobRestorer>,
		),
		(
			<FileDecryptorJob as StatefulJob>::NAME,
			Box::new(FileDecryptorJob {}) as Box<dyn JobRestorer>,
		),
	])
});

/// TODO
pub struct QueuedJob {
	// /// name of the job. This comes from `StatefulJob::NAME`.
	// name: &'static str,
	/// the job report.
	report: JobReport,
	/// channel which is used to signal the job thread to start execution
	start: oneshot::Sender<()>,
}

/// TODO
pub struct RunningJob {
	/// the job report. NOTE: THIS IS EVENTUALLY CONSISTENT
	/// Every `STEPS_BETWEEN_PERSIST` steps the job report here will be updated.
	report: JobReport,
	// TODO: Pause channel?
}

/// TODO
pub struct JobManager {
	/// jobs that are currently running
	running: RwLock<HashMap<Uuid, RunningJob>>,
	/// jobs that are queued and waiting to be run
	queue: RwLock<VecDeque<QueuedJob>>,
	/// a channel used to shutdown all of the worker threads
	shutdown_tx: broadcast::Sender<()>, // TODO: When `JobManager` drops does this also drop all worker threads?
}

impl JobManager {
	/// TODO
	pub fn new() -> Arc<Self> {
		// We ignore `_shutdown_rx` because it's a broadcast channel
		let (shutdown_tx, _shutdown_rx) = broadcast::channel(1);
		Arc::new(Self {
			running: RwLock::new(Default::default()),
			queue: RwLock::new(Default::default()),
			shutdown_tx,
		})
	}

	/// TODO
	pub async fn ingest<T: StatefulJob>(
		self: Arc<Self>,
		ctx: LibraryContext,
		init: T::Init,
		job: T,
	) {
		if !JOB_RESTORER.contains_key(T::NAME) {
			error!("Job of type '{}' was ingested but does not have a restorer configured in the `JobManager`. Ignoring this as an invalid request!", T::NAME);
			return;
		}

		self.internal_dispatch_job(
			ctx,
			JobReport::new(Uuid::new_v4(), T::NAME.to_string()), // `dispatch_job` will handle pushing this to the database
			JobState {
				init,
				data: None,
				steps: VecDeque::new(),
				step_number: 0,
			},
			job,
		)
		.await;
	}

	/// TODO
	pub async fn get_running(&self) -> Vec<JobReport> {
		self.running
			.read()
			.await
			.iter()
			.map(|(_, v)| v.report.clone())
			.collect()
	}

	/// TODO
	pub async fn clear_all_jobs(
		ctx: &LibraryContext,
	) -> Result<(), prisma_client_rust::QueryError> {
		// TODO: Delete running jobs??? -> Upsert in the job itself will recreate them

		ctx.db.job().delete_many(vec![]).exec().await?;
		invalidate_query!(ctx, "jobs.getHistory");
		Ok(())
	}

	/// TODO
	pub async fn pause(&self) {
		self.shutdown_tx
			.send(())
			.expect("Failed to send shutdown signal");

		loop {
			sleep(Duration::from_millis(50)).await;
			if self.running.read().await.is_empty() {
				break;
			}
		}

		todo!();
	}

	/// TODO
	pub async fn resume_jobs(self: &Arc<Self>, ctx: &LibraryContext) -> Result<(), JobError> {
		let paused_jobs = ctx
			.db
			.job()
			.find_many(vec![or!(
				job::status::equals(JobStatus::Paused.int_value()),
				job::status::equals(JobStatus::Running.int_value()), // Will occur if the core crashes as the last saved state will be running
			)])
			.exec()
			.await?;

		// TODO: If we error out we are gonna end up with everything half loaded into memory which is bad. Deal with this!
		for job in paused_jobs {
			let mut report = JobReport::from(job);

			let job_state_data = if let Some(data) = report.data.take() {
				data
			} else {
				// TODO: What about this optional? Can it be removed by changing the Prisma Schema?
				return Err(JobError::MissingJobDataState(report.id, report.name));
			};

			info!("Resuming job: {}, id: {}", report.name, report.id);

			match JOB_RESTORER.get(report.name.as_str()) {
				Some(restorer) => {
					restorer
						.restore(Arc::clone(self), ctx, report, job_state_data)
						.await?;
				}
				None => {
					error!(
						"Error restoring job of id '{}' due it being of an unknown type '{}'",
						report.id, report.name
					);
					return Err(JobError::UnknownJobName(report.id, report.name));
				}
			}
		}

		Ok(())
	}

	/// TODO
	/// This should be treated as a private function but it is public because it is used within implementations of the `JobRestorer` trait.
	pub async fn internal_dispatch_job<T: StatefulJob>(
		self: Arc<Self>,
		library_ctx: LibraryContext,
		mut report: JobReport,
		mut state: JobState<T>,
		job: T,
	) {
		let job_should_queue = self.running.read().await.len() <= MAX_WORKERS;
		report.status = job_should_queue
			.then(|| JobStatus::Running)
			.unwrap_or(JobStatus::Queued);
		report.upsert(&library_ctx).await.unwrap(); // TODO: Error handling

		tokio::spawn(async move {
			let mut shutdown_rx = self.shutdown_tx.subscribe();
			if job_should_queue {
				let (start_tx, start_rx) = oneshot::channel();
				self.queue.write().await.push_back(QueuedJob {
					report: report.clone(),
					start: start_tx,
				});
				debug!("Queueing job '{}'", T::NAME);

				// Await the job start signal or system shutdown
				tokio::select! {
					biased;
					_ = shutdown_rx.recv() => {
						// TODO: Persist job state to the database
						// rmp_serde::to_vec_named(&self.state)?;

						report.upsert(&library_ctx).await.unwrap(); // TODO: Error handling
						self.running.write().await.remove(&report.id);
						return;
					}
					_ = start_rx => {},
				};

				report.status = JobStatus::Running;
				report.upsert(&library_ctx).await.unwrap(); // TODO: Error handling
			} else {
				self.running.write().await.insert(
					report.id,
					RunningJob {
						report: report.clone(),
					},
				);
			}
			info!("Running job '{}'", T::NAME);

			let mut ctx = WorkerContext {
				report,
				library_ctx,
				shutdown_tx: self.shutdown_tx.clone(),
			};

			// TODO: Mark job as initialising into the manager

			// Checking if we have a brand new job, or if we are resuming an old one.
			if state.data.is_none() {
				job.init(&mut ctx, &mut state).await.unwrap(); // TODO: Error handling
			}

			let mut last_update = 0;
			while !state.steps.is_empty() {
				tokio::select! {
					step_result = job.execute_step(&mut ctx, &mut state) => {
						match step_result {
							Ok(_) => { state.steps.pop_front(); },
							Err(JobError::EarlyFinish(reason)) => {
								warn!("Job '{}' had a early finish: {}", T::NAME, reason);
								break;
							}
							Err(err) => {
								warn!("Job '{}' encountered an error: {}", T::NAME, err);
								break;
							}
						}
					}
					_ = shutdown_rx.recv() => {
						// TODO: Persist job state to the database
						// rmp_serde::to_vec_named(&self.state)?;

						ctx.report.upsert(&ctx.library_ctx).await.unwrap(); // TODO: Error handling
						self.running.write().await.remove(&ctx.report.id);
						return;
					}
				}

				if last_update == STEPS_BETWEEN_PERSIST {
					last_update = 0;
					// TODO: Write job state as well???
					ctx.report.upsert(&ctx.library_ctx).await.unwrap(); // TODO: Error handling
					self.running.write().await.insert(
						ctx.report.id,
						RunningJob {
							report: ctx.report.clone(),
						},
					);

					invalidate_query!(ctx.library_ctx, "jobs.getRunning");
				} else {
					last_update += 1;
				}

				state.step_number += 1;
			}

			match job.finalize(&mut ctx, &mut state).await {
				Ok(metadata) => {
					info!("Completed job '{}' with id '{}'", T::NAME, ctx.report.id);
					ctx.report.status = JobStatus::Completed;
					ctx.report.data = None;
					ctx.report.metadata = metadata;
				}
				Err(JobError::Paused(state)) => {
					info!("Paused job '{}' with id '{}'", T::NAME, ctx.report.id);
					ctx.report.status = JobStatus::Paused;
					ctx.report.data = Some(state);
				}
				Err(err) => {
					warn!(
						"Error occurred running job '{}' with id '{}': {}",
						T::NAME,
						ctx.report.id,
						err
					);
					ctx.report.status = JobStatus::Failed;
					ctx.report.data = None;
				}
			}

			// persist the job report, remove current job from running and start next job
			if let Err(err) = ctx.report.upsert(&ctx.library_ctx).await {
				error!("failed to upsert job report: {:#?}", err);
			}

			{
				let next_job = self.queue.write().await.pop_front();
				let mut running = self.running.write().await;
				running.remove(&ctx.report.id);
				if let Some(next_job) = next_job {
					let id = next_job.report.id.clone();
					let name = next_job.report.name.clone();
					running.insert(
						next_job.report.id,
						RunningJob {
							report: next_job.report,
						},
					);
					if let Err(err) = next_job.start.send(()) {
						error!(
							"failed to trigger job '{}' with id '{}': {:#?}",
							name, id, err
						);
					}
				}
			}
			invalidate_query!(ctx.library_ctx, "jobs.getRunning");
		});
	}
}
