use crate::{
	library::Library,
	location::indexer::IndexerError,
	object::{file_identifier::FileIdentifierJobError, preview::ThumbnailerError},
};

use std::{
	collections::{hash_map::DefaultHasher, VecDeque},
	fmt::Debug,
	hash::{Hash, Hasher},
	path::PathBuf,
	sync::Arc,
};

use rmp_serde::{decode::Error as DecodeError, encode::Error as EncodeError};
use sd_crypto::Error as CryptoError;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use thiserror::Error;
use tracing::{debug, error, info};
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
	#[error("error converting/handling OS strings")]
	OsStr,
	#[error("error converting/handling paths")]
	Path,
	#[error("invalid job status integer")]
	InvalidJobStatusInt(i32),

	// Specific job errors
	#[error("Indexer error: {0}")]
	IndexerError(#[from] IndexerError),
	#[error("Thumbnailer error: {0}")]
	ThumbnailError(#[from] ThumbnailerError),
	#[error("Identifier error: {0}")]
	IdentifierError(#[from] FileIdentifierJobError),
	#[error("Crypto error: {0}")]
	CryptoError(#[from] CryptoError),
	#[error("source and destination path are the same: {}", .0.display())]
	MatchingSrcDest(PathBuf),
	#[error("action would overwrite another file: {}", .0.display())]
	WouldOverwrite(PathBuf),

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

/// `JobInitData` is a trait to represent the data being passed to initialize a `Job`
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
}

#[async_trait::async_trait]
pub trait DynJob: Send + Sync {
	fn id(&self) -> Uuid;
	fn parent_id(&self) -> Option<Uuid>;
	fn report(&self) -> &Option<JobReport>;
	fn report_mut(&mut self) -> &mut Option<JobReport>;
	fn name(&self) -> &'static str;
	async fn run(&mut self, job_manager: Arc<JobManager>, ctx: WorkerContext) -> JobResult;
	fn hash(&self) -> u64;
	fn queue_next(&mut self, next_job: Box<dyn DynJob>);
	fn serialize_state(&self) -> Result<Vec<u8>, JobError>;
	async fn register_children(&mut self, library: &Library) -> Result<(), JobError>;
	async fn pause_children(&mut self, library: &Library) -> Result<(), JobError>;
	async fn cancel_children(&mut self, library: &Library) -> Result<(), JobError>;
}

pub struct Job<SJob: StatefulJob> {
	report: Option<JobReport>,
	state: JobState<SJob>,
	stateful_job: SJob,
	next_job: Option<Box<dyn DynJob>>,
}

pub trait IntoJob<SJob: StatefulJob + 'static> {
	fn into_job(self) -> Box<dyn DynJob>;
}

impl<SJob, Init> IntoJob<SJob> for Init
where
	SJob: StatefulJob<Init = Init> + 'static,
	Init: JobInitData<Job = SJob>,
{
	fn into_job(self) -> Box<dyn DynJob> {
		Job::new(self)
	}
}

impl<SJob, Init> IntoJob<SJob> for Box<Job<SJob>>
where
	SJob: StatefulJob<Init = Init> + 'static,
	Init: JobInitData<Job = SJob>,
{
	fn into_job(self) -> Box<dyn DynJob> {
		self
	}
}

impl<SJob, Init> Job<SJob>
where
	SJob: StatefulJob<Init = Init> + 'static,
	Init: JobInitData<Job = SJob>,
{
	pub fn new(init: Init) -> Box<Self> {
		Box::new(Self {
			report: Some(JobReport::new(Uuid::new_v4(), SJob::NAME.to_string())),
			state: JobState {
				init,
				data: None,
				steps: VecDeque::new(),
				step_number: 0,
			},
			stateful_job: SJob::new(),
			next_job: None,
		})
	}

	pub fn queue_next<NextSJob, NextInit>(mut self: Box<Self>, init: NextInit) -> Box<Self>
	where
		NextSJob: StatefulJob<Init = NextInit> + 'static,
		NextInit: JobInitData<Job = NextSJob>,
	{
		let last_job = Job::new_dependent(
			init,
			self.next_job
				.as_ref()
				.map(|job| job.id())
				// SAFETY: If we're queueing a next job then we should have a report yet
				.unwrap_or(self.report.as_ref().unwrap().id),
		);

		if let Some(ref mut next) = self.next_job {
			next.queue_next(last_job);
		} else {
			self.next_job = Some(last_job);
		}

		self
	}

	pub fn resume(
		mut report: JobReport,
		stateful_job: SJob,
		next_job: Option<Box<dyn DynJob>>,
	) -> Result<Box<dyn DynJob>, JobError> {
		let job_state_data = if let Some(data) = report.data.take() {
			data
		} else {
			return Err(JobError::MissingJobDataState(report.id, report.name));
		};

		Ok(Box::new(Self {
			report: Some(report),
			state: rmp_serde::from_slice(&job_state_data)?,
			stateful_job,
			next_job,
		}))
	}

	fn new_dependent(init: Init, parent_id: Uuid) -> Box<Self> {
		Box::new(Self {
			report: Some(JobReport::new_with_parent(
				Uuid::new_v4(),
				SJob::NAME.to_string(),
				parent_id,
			)),
			state: JobState {
				init,
				data: None,
				steps: VecDeque::new(),
				step_number: 0,
			},
			stateful_job: SJob::new(),
			next_job: None,
		})
	}
}

#[derive(Serialize, Deserialize)]
pub struct JobState<Job: StatefulJob> {
	pub init: Job::Init,
	pub data: Option<Job::Data>,
	pub steps: VecDeque<Job::Step>,
	pub step_number: usize,
}

#[async_trait::async_trait]
impl<SJob: StatefulJob> DynJob for Job<SJob> {
	fn id(&self) -> Uuid {
		// SAFETY: This method is using during queueing, so we still have a report
		self.report().as_ref().unwrap().id
	}

	fn parent_id(&self) -> Option<Uuid> {
		self.report.as_ref().and_then(|r| r.parent_id)
	}

	fn report(&self) -> &Option<JobReport> {
		&self.report
	}

	fn report_mut(&mut self) -> &mut Option<JobReport> {
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

		if let Some(next_job) = self.next_job.take() {
			debug!(
				"Job '{}' requested to spawn '{}' now that it's complete!",
				self.name(),
				next_job.name()
			);

			if let Err(e) = job_manager.clone().ingest(&ctx.library, next_job).await {
				error!("Failed to ingest next job: {e}");
			}
		}

		Ok(metadata)
	}

	fn hash(&self) -> u64 {
		<SJob::Init as JobInitData>::hash(&self.state.init)
	}

	fn queue_next(&mut self, next_job: Box<dyn DynJob>) {
		if let Some(ref mut next) = self.next_job {
			next.queue_next(next_job);
		} else {
			self.next_job = Some(next_job);
		}
	}

	fn serialize_state(&self) -> Result<Vec<u8>, JobError> {
		rmp_serde::to_vec_named(&self.state).map_err(Into::into)
	}

	async fn register_children(&mut self, library: &Library) -> Result<(), JobError> {
		if let Some(ref mut next_job) = self.next_job {
			// SAFETY: As these children jobs haven't been run yet, they still have their report field
			let next_job_report = next_job.report_mut().as_mut().unwrap();
			if next_job_report.created_at.is_none() {
				next_job_report.create(library).await?
			}

			next_job.register_children(library).await?;
		}

		Ok(())
	}

	async fn pause_children(&mut self, library: &Library) -> Result<(), JobError> {
		if let Some(ref mut next_job) = self.next_job {
			let state = next_job.serialize_state()?;

			// SAFETY: As these children jobs haven't been run yet, they still have their report field
			let mut report = next_job.report_mut().as_mut().unwrap();
			report.status = JobStatus::Paused;
			report.data = Some(state);
			report.update(library).await?;
			next_job.pause_children(library).await?;
		}

		Ok(())
	}

	async fn cancel_children(&mut self, library: &Library) -> Result<(), JobError> {
		if let Some(ref mut next_job) = self.next_job {
			let state = next_job.serialize_state()?;

			// SAFETY: As these children jobs haven't been run yet, they still have their report field
			let mut report = next_job.report_mut().as_mut().unwrap();
			report.status = JobStatus::Canceled;
			report.data = Some(state);
			report.update(library).await?;
			next_job.cancel_children(library).await?;
		}

		Ok(())
	}
}
