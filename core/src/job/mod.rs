use crate::{
	location::{indexer::IndexerError, LocationError},
	object::{identifier_job::IdentifierJobError, preview::ThumbnailError},
};

use std::{
	collections::{hash_map::DefaultHasher, VecDeque},
	fmt::Debug,
	hash::{Hash, Hasher},
};

use rmp_serde::{decode::Error as DecodeError, encode::Error as EncodeError};
use sd_crypto::Error as CryptoError;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use thiserror::Error;
use tracing::warn;
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

	// Specific job errors
	#[error("Indexer error: {0}")]
	IndexerError(#[from] IndexerError),
	#[error("Location error: {0}")]
	LocationError(#[from] LocationError),
	#[error("Thumbnail error: {0}")]
	ThumbnailError(#[from] ThumbnailError),
	#[error("Identifier error: {0}")]
	IdentifierError(#[from] IdentifierJobError),

	// Not errors
	#[error("Job had a early finish: <name='{name}', reason='{reason}'>")]
	EarlyFinish { name: String, reason: String },
	#[error("Crypto error: {0}")]
	CryptoError(#[from] CryptoError),
	#[error("Data needed for job execution not found: job <name='{0}'>")]
	JobDataNotFound(String),
	#[error("Job paused")]
	Paused(Vec<u8>),
}

pub type JobResult = Result<JobMetadata, JobError>;
pub type JobMetadata = Option<serde_json::Value>;

#[async_trait::async_trait]
pub trait StatefulJob: Send + Sync + Sized {
	type Init: Serialize + DeserializeOwned + Send + Sync + Hash;
	type Data: Serialize + DeserializeOwned + Send + Sync;
	type Step: Serialize + DeserializeOwned + Send + Sync;

	fn name(&self) -> &'static str;
	async fn init(
		&self,
		ctx: &mut WorkerContext,
		state: &mut JobState<Self>,
	) -> Result<(), JobError>;

	async fn execute_step(
		&self,
		ctx: &mut WorkerContext,
		state: &mut JobState<Self>,
	) -> Result<(), JobError>;

	async fn finalize(&self, ctx: &mut WorkerContext, state: &mut JobState<Self>) -> JobResult;
}

#[async_trait::async_trait]
pub trait DynJob: Send + Sync {
	fn report(&mut self) -> &mut Option<JobReport>;
	fn name(&self) -> &'static str;
	async fn run(&mut self, ctx: &mut WorkerContext) -> JobResult;
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
				stateful_job.name().to_string(),
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

impl<State: StatefulJob> Hash for Job<State> {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.name().hash(state);
		self.state.hash(state);
	}
}

#[derive(Serialize, Deserialize)]
pub struct JobState<Job: StatefulJob> {
	pub init: Job::Init,
	pub data: Option<Job::Data>,
	pub steps: VecDeque<Job::Step>,
	pub step_number: usize,
}

impl<Job: StatefulJob> Hash for JobState<Job> {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.init.hash(state);
	}
}

#[async_trait::async_trait]
impl<State: StatefulJob> DynJob for Job<State> {
	fn report(&mut self) -> &mut Option<JobReport> {
		&mut self.report
	}

	fn name(&self) -> &'static str {
		self.stateful_job.name()
	}

	async fn run(&mut self, ctx: &mut WorkerContext) -> JobResult {
		// Checking if we have a brand new job, or if we are resuming an old one.
		if self.state.data.is_none() {
			self.stateful_job.init(ctx, &mut self.state).await?;
		}

		let mut shutdown_rx = ctx.shutdown_rx();
		let shutdown_rx_fut = shutdown_rx.recv();
		tokio::pin!(shutdown_rx_fut);

		while !self.state.steps.is_empty() {
			tokio::select! {
				step_result = self.stateful_job.execute_step(
					ctx,
					&mut self.state,
				) => {
					if matches!(step_result, Err(JobError::EarlyFinish { .. })) {
						warn!("{}", step_result.unwrap_err());
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

		self.stateful_job.finalize(ctx, &mut self.state).await
	}

	fn hash(&self) -> u64 {
		let mut hasher = DefaultHasher::new();
		Hash::hash(self, &mut hasher);
		hasher.finish()
	}
}
