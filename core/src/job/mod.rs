use crate::{prisma, sys::LocationError};
use rmp_serde::{decode::Error as DecodeError, encode::Error as EncodeError};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{collections::VecDeque, fmt::Debug};
use thiserror::Error;
use uuid::Uuid;

mod job_manager;
mod worker;

pub use job_manager::*;
pub use worker::*;

#[derive(Error, Debug)]
pub enum JobError {
	#[error("Database error: {0}")]
	DatabaseError(#[from] prisma::QueryError),
	#[error("Location error: {0}")]
	LocationError(#[from] LocationError),
	#[error("I/O error: {0}")]
	IOError(#[from] std::io::Error),
	#[error("Failed to join Tokio spawn blocking: {0}")]
	JoinError(#[from] tokio::task::JoinError),
	#[error("Job state encode error: {0}")]
	StateEncode(#[from] EncodeError),
	#[error("Job state decode error: {0}")]
	StateDecode(#[from] DecodeError),
	#[error("Tried to resume a job with unknown name: job <name='{1}', uuid='{0}'>")]
	UnknownJobName(Uuid, String),
	#[error(
		"Tried to resume a job that doesn't have saved state data: job <name='{1}', uuid='{0}'>"
	)]
	MissingJobDataState(Uuid, String),
	#[error("Job paused")]
	Paused(Vec<u8>),
}

pub type JobResult = Result<(), JobError>;

#[async_trait::async_trait]
pub trait StatefulJob: Send + Sync {
	type Init: Serialize + DeserializeOwned + Send + Sync;
	type Data: Serialize + DeserializeOwned + Send + Sync;
	type Step: Serialize + DeserializeOwned + Send + Sync;

	fn name(&self) -> &'static str;
	async fn init(
		&self,
		ctx: WorkerContext,
		state: &mut JobState<Self::Init, Self::Data, Self::Step>,
	) -> JobResult;

	async fn execute_step(
		&self,
		ctx: WorkerContext,
		state: &mut JobState<Self::Init, Self::Data, Self::Step>,
	) -> JobResult;

	async fn finalize(
		&self,
		ctx: WorkerContext,
		state: &mut JobState<Self::Init, Self::Data, Self::Step>,
	) -> JobResult;
}

#[async_trait::async_trait]
pub trait DynJob: Send + Sync {
	fn report(&mut self) -> &mut Option<JobReport>;
	fn name(&self) -> &'static str;
	async fn run(&mut self, ctx: WorkerContext) -> JobResult;
}

pub struct Job<Init, Data, Step>
where
	Init: Serialize + DeserializeOwned + Send + Sync,
	Data: Serialize + DeserializeOwned + Send + Sync,
	Step: Serialize + DeserializeOwned + Send + Sync,
{
	report: Option<JobReport>,
	state: JobState<Init, Data, Step>,
	stateful_job: Box<dyn StatefulJob<Init = Init, Data = Data, Step = Step>>,
}

impl<Init, Data, Step> Job<Init, Data, Step>
where
	Init: Serialize + DeserializeOwned + Send + Sync,
	Data: Serialize + DeserializeOwned + Send + Sync,
	Step: Serialize + DeserializeOwned + Send + Sync,
{
	pub fn new(
		init: Init,
		stateful_job: Box<dyn StatefulJob<Init = Init, Data = Data, Step = Step>>,
	) -> Box<Self> {
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

	pub fn resume(
		mut report: JobReport,
		stateful_job: Box<dyn StatefulJob<Init = Init, Data = Data, Step = Step>>,
	) -> Result<Box<Self>, JobError> {
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

#[derive(Serialize, Deserialize)]
pub struct JobState<Init, Data, Step> {
	pub init: Init,
	pub data: Option<Data>,
	pub steps: VecDeque<Step>,
	pub step_number: usize,
}

#[async_trait::async_trait]
impl<Init, Data, Step> DynJob for Job<Init, Data, Step>
where
	Init: Serialize + DeserializeOwned + Send + Sync,
	Data: Serialize + DeserializeOwned + Send + Sync,
	Step: Serialize + DeserializeOwned + Send + Sync,
{
	fn report(&mut self) -> &mut Option<JobReport> {
		&mut self.report
	}

	fn name(&self) -> &'static str {
		self.stateful_job.name()
	}
	async fn run(&mut self, ctx: WorkerContext) -> JobResult {
		// Checking if we have a brand new job, or if we are resuming an old one.
		if self.state.data.is_none() {
			self.stateful_job.init(ctx.clone(), &mut self.state).await?;
		}

		let mut shutdown_rx = ctx.shutdown_rx();
		let shutdown_rx_fut = shutdown_rx.recv();
		tokio::pin!(shutdown_rx_fut);

		while !self.state.steps.is_empty() {
			tokio::select! {
				step_result = self.stateful_job.execute_step(
					ctx.clone(),
					&mut self.state,
				) => {
					step_result?;
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

		self.stateful_job
			.finalize(ctx.clone(), &mut self.state)
			.await?;

		Ok(())
	}
}
