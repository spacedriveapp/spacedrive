use crate::{
	location::{indexer::IndexerError, LocationError, LocationManagerError},
	object::{file_identifier::FileIdentifierJobError, preview::ThumbnailerError},
};

use std::{
	collections::{hash_map::DefaultHasher, VecDeque},
	fmt::Debug,
	hash::{Hash, Hasher},
	sync::Arc,
};

use rmp_serde::{decode::Error as DecodeError, encode::Error as EncodeError};
use sd_crypto::Error as CryptoError;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use thiserror::Error;
use tracing::{debug, info};
use uuid::Uuid;

mod job_manager;
mod worker;

pub use job_manager::*;
pub use worker::*;

#[derive(Error, Debug)]
pub enum JobError {
	// General errors
	#[error("Database error: {0}")]
	DatabaseError(#[from] prisma_client_rust::QueryError),
	#[error("I/O error: {0}")]
	IOError(#[from] std::io::Error),
	#[error("Failed to join Tokio spawn blocking: {0}")]
	JoinTaskError(#[from] tokio::task::JoinError),
	#[error("Job state encode error: {0}")]
	StateEncode(#[from] EncodeError),
	#[error("Job state decode error: {0}")]
	StateDecode(#[from] DecodeError),
	#[error("Job metadata serialization error: {0}")]
	MetadataSerialization(#[from] serde_json::Error),
	#[error("Tried to resume a job with unknown name: job <name='{1}', uuid='{0}'>")]
	UnknownJobName(Uuid, String),
	#[error(
		"Tried to resume a job that doesn't have saved state data: job <name='{1}', uuid='{0}'>"
	)]
	MissingJobDataState(Uuid, String),
	#[error("missing some job data: '{value}'")]
	MissingData { value: String },
	#[error("Location manager error: {0}")]
	LocationManager(#[from] LocationManagerError),
	#[error("error converting/handling OS strings")]
	OsStr,
	#[error("error converting/handling paths")]
	Path,

	// Specific job errors
	#[error("Indexer error: {0}")]
	IndexerError(#[from] IndexerError),
	#[error("Location error: {0}")]
	LocationError(#[from] LocationError),
	#[error("Thumbnailer error: {0}")]
	ThumbnailError(#[from] ThumbnailerError),
	#[error("Identifier error: {0}")]
	IdentifierError(#[from] FileIdentifierJobError),
	#[error("Crypto error: {0}")]
	CryptoError(#[from] CryptoError),

	// Not errors
	#[error("Job had a early finish: <name='{name}', reason='{reason}'>")]
	EarlyFinish { name: String, reason: String },
	#[error("Data needed for job execution not found: job <name='{0}'>")]
	JobDataNotFound(String),
	#[error("Job paused")]
	Paused(Vec<u8>),
}

pub type JobResult = Result<JobMetadata, JobError>;
pub type JobMetadata = Option<serde_json::Value>;

/// TODO
pub trait JobInitData: Serialize + DeserializeOwned + Send + Sync + Hash {
	type Job: StatefulJob;

	fn hash(&self) -> u64 {
		let mut s = DefaultHasher::new();
		<Self::Job as StatefulJob>::NAME.hash(&mut s);
		<Self as Hash>::hash(self, &mut s);
		s.finish()
	}
}

#[async_trait::async_trait]
pub trait StatefulJob: Send + Sync + Sized {
	type Init: JobInitData<Job = Self>;
	type Data: Serialize + DeserializeOwned + Send + Sync;
	type Step: Serialize + DeserializeOwned + Send + Sync;

	/// The name of the job is a unique human readable identifier for the job.
	const NAME: &'static str;

	/// Construct a new instance of the job. This is used so the user can pass `Self::Init` into the `spawn_job` function and we can still run the job.
	/// This does remove the flexibility of being able to pass arguments into the job's struct but with resumable jobs I view that as an anti-pattern anyway.
	fn new() -> Self;

	/// initialize the steps for the job
	async fn init(&self, ctx: WorkerContext, state: &mut JobState<Self>) -> Result<(), JobError>;

	/// is called for each step in the job. These steps are created in the `Self::init` method.
	async fn execute_step(
		&self,
		ctx: WorkerContext,
		state: &mut JobState<Self>,
	) -> Result<(), JobError>;

	/// is called after all steps have been executed
	async fn finalize(&mut self, ctx: WorkerContext, state: &mut JobState<Self>) -> JobResult;

	/// allows a job to queue up more jobs which are executed after it has finished successfully
	/// WARNING: This method can be called multiple times which may be before the job finishes.
	/// WARNING: This is because the UI needs to display the queued jobs.
	/// WARNING: It MUST always call `ctx.spawn_job` with the same arguments and the same order.
	fn queue_jobs(&self, ctx: &mut QueueJobsCtx, state: &mut JobState<Self>) {
		let _ = (ctx, state);
	}
}

pub struct QueueJobsCtx {
	jobs: Vec<Box<dyn DynJob>>,
}

impl QueueJobsCtx {
	pub(crate) fn spawn_job<
		J: StatefulJob<Init = TInitData> + 'static,
		TInitData: JobInitData<Job = J>,
	>(
		&mut self,
		init: TInitData,
	) {
		self.jobs.push(Job::new(init, J::new()));
	}
}

#[async_trait::async_trait]
pub trait DynJob: Send + Sync {
	fn report(&mut self) -> &mut Option<JobReport>;
	fn name(&self) -> &'static str;
	async fn run(&mut self, job_manager: Arc<JobManager>, ctx: WorkerContext) -> JobResult;
	fn hash(&self) -> u64;
}

pub struct Job<SJob: StatefulJob> {
	report: Option<JobReport>,
	state: JobState<SJob>,
	stateful_job: SJob,
}

impl<SJob: StatefulJob> Job<SJob> {
	pub fn new(init: SJob::Init, stateful_job: SJob) -> Box<Self> {
		Box::new(Self {
			report: Some(JobReport::new(
				Uuid::new_v4(),
				<SJob as StatefulJob>::NAME.to_string(),
			)),
			state: JobState {
				init,
				data: None,
				steps: VecDeque::new(),
				step_number: 0,
			},
			stateful_job,
		})
	}

	pub fn resume(mut report: JobReport, stateful_job: SJob) -> Result<Box<Self>, JobError> {
		let job_state_data = if let Some(data) = report.data.take() {
			data
		} else {
			return Err(JobError::MissingJobDataState(report.id, report.name));
		};

		Ok(Box::new(Self {
			report: Some(report),
			state: rmp_serde::from_slice(&job_state_data)?,
			stateful_job,
		}))
	}
}

// impl<State: StatefulJob> Hash for Job<State> {
// 	fn hash<H: Hasher>(&self, state: &mut H) {
// 		self.name().hash(state);
// 		self.state.hash(state);
// 	}
// }

#[derive(Serialize, Deserialize)]
pub struct JobState<Job: StatefulJob> {
	pub init: Job::Init,
	pub data: Option<Job::Data>,
	pub steps: VecDeque<Job::Step>,
	pub step_number: usize,
}

// impl<Job: StatefulJob> Hash for JobState<Job> {
// 	fn hash<H: Hasher>(&self, state: &mut H) {
// 		<Self as JobInitData>::hash(state);
// 	}
// }

#[async_trait::async_trait]
impl<SJob: StatefulJob> DynJob for Job<SJob> {
	fn report(&mut self) -> &mut Option<JobReport> {
		&mut self.report
	}

	fn name(&self) -> &'static str {
		<SJob as StatefulJob>::NAME
	}

	async fn run(&mut self, job_manager: Arc<JobManager>, ctx: WorkerContext) -> JobResult {
		let mut job_should_run = true;

		// Checking if we have a brand new job, or if we are resuming an old one.
		if self.state.data.is_none() {
			if let Err(e) = self.stateful_job.init(ctx.clone(), &mut self.state).await {
				if matches!(e, JobError::EarlyFinish { .. }) {
					info!("{e}");
					job_should_run = false;
				} else {
					return Err(e);
				}
			}
		}

		let mut shutdown_rx = ctx.shutdown_rx();
		let shutdown_rx_fut = shutdown_rx.recv();
		tokio::pin!(shutdown_rx_fut);

		while job_should_run && !self.state.steps.is_empty() {
			tokio::select! {
				step_result = self.stateful_job.execute_step(
					ctx.clone(),
					&mut self.state,
				) => {
					if matches!(step_result, Err(JobError::EarlyFinish { .. })) {
						info!("{}", step_result.unwrap_err());
						break;
					} else {
						step_result?;
					};
					self.state.steps.pop_front();
				}
				_ = &mut shutdown_rx_fut => {
					return Err(
						JobError::Paused(
							rmp_serde::to_vec_named(&self.state)?
						)
					);
				}
			}
			self.state.step_number += 1;
		}

		let metadata = self
			.stateful_job
			.finalize(ctx.clone(), &mut self.state)
			.await?;

		let mut queue_ctx = QueueJobsCtx { jobs: Vec::new() };
		self.stateful_job
			.queue_jobs(&mut queue_ctx, &mut self.state);

		for job in queue_ctx.jobs {
			debug!(
				"Job '{}' requested to spawn '{}' now that it's complete!",
				self.name(),
				job.name()
			);

			job_manager.clone().ingest(&ctx.library, job).await;
		}

		Ok(metadata)
	}

	fn hash(&self) -> u64 {
		<SJob::Init as JobInitData>::hash(&self.state.init)
	}
}
