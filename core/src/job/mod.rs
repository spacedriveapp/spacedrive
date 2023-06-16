use crate::library::Library;

use std::{
	collections::{hash_map::DefaultHasher, VecDeque},
	hash::{Hash, Hasher},
	mem,
	sync::{atomic::Ordering, Arc},
	time::Duration,
};

use serde::{de::DeserializeOwned, Deserialize, Serialize};

use tracing::{debug, error, info, warn};
use uuid::Uuid;

mod error;
mod manager;
mod report;
mod worker;

pub use error::*;
pub use manager::*;
pub use report::*;
pub use worker::*;

pub type JobResult = Result<JobMetadata, JobError>;
pub type JobMetadata = Option<serde_json::Value>;
pub type JobRunErrors = Vec<String>;
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
	const IS_BACKGROUND: bool = false;

	/// Construct a new instance of the job. This is used so the user can pass `Self::Init` into the `spawn_job` function and we can still run the job.
	/// This does remove the flexibility of being able to pass arguments into the job's struct but with resumable jobs I view that as an anti-pattern anyway.
	fn new() -> Self;

	/// initialize the steps for the job
	async fn init(
		&self,
		ctx: &mut WorkerContext,
		state: &mut JobState<Self>,
	) -> Result<(), JobError>;

	/// is called for each step in the job. These steps are created in the `Self::init` method.
	async fn execute_step(
		&self,
		ctx: &mut WorkerContext,
		state: &mut JobState<Self>,
	) -> Result<(), JobError>;

	/// is called after all steps have been executed
	async fn finalize(&mut self, ctx: &mut WorkerContext, state: &mut JobState<Self>) -> JobResult;
}

#[async_trait::async_trait]
pub trait DynJob: Send + Sync {
	fn id(&self) -> Uuid;
	fn parent_id(&self) -> Option<Uuid>;
	fn report(&self) -> &Option<JobReport>;
	fn report_mut(&mut self) -> &mut Option<JobReport>;
	fn name(&self) -> &'static str;
	async fn run(
		&mut self,
		job_manager: Arc<JobManager>,
		ctx: &mut WorkerContext,
	) -> Result<(JobMetadata, JobRunErrors), JobError>;
	fn hash(&self) -> u64;
	fn set_next_jobs(&mut self, next_jobs: VecDeque<Box<dyn DynJob>>);
	fn serialize_state(&self) -> Result<Vec<u8>, JobError>;
	async fn register_children(&mut self, library: &Library) -> Result<(), JobError>;
	async fn pause_children(&mut self, library: &Library) -> Result<(), JobError>;
	async fn cancel_children(&mut self, library: &Library) -> Result<(), JobError>;
}

pub struct Job<SJob: StatefulJob> {
	id: Uuid,
	report: Option<JobReport>,
	state: JobState<SJob>,
	stateful_job: SJob,
	next_jobs: VecDeque<Box<dyn DynJob>>,
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
	fn new(init: Init) -> Box<Self> {
		let id = Uuid::new_v4();
		Box::new(Self {
			id,
			report: Some(JobReport::new(id, SJob::NAME.to_string())),
			state: JobState {
				init,
				data: None,
				steps: VecDeque::new(),
				step_number: 0,
			},
			stateful_job: SJob::new(),
			next_jobs: VecDeque::new(),
		})
	}

	pub fn new_with_action(init: Init, action: impl AsRef<str>) -> Box<Self> {
		let id = Uuid::new_v4();
		Box::new(Self {
			id,
			report: Some(JobReport::new_with_action(
				id,
				SJob::NAME.to_string(),
				action,
			)),
			state: JobState {
				init,
				data: None,
				steps: VecDeque::new(),
				step_number: 0,
			},
			stateful_job: SJob::new(),
			next_jobs: VecDeque::new(),
		})
	}

	pub fn queue_next<NextSJob, NextInit>(mut self: Box<Self>, init: NextInit) -> Box<Self>
	where
		NextSJob: StatefulJob<Init = NextInit> + 'static,
		NextInit: JobInitData<Job = NextSJob>,
	{
		let next_job_order = self.next_jobs.len() + 1;
		self.next_jobs.push_back(Job::new_dependent(
			init,
			self.id,
			// SAFETY: If we're queueing a next job then we should still have a report
			self.report().as_ref().and_then(|parent_report| {
				parent_report
					.action
					.as_ref()
					.map(|parent_action| format!("{parent_action}-{next_job_order}"))
			}),
		));

		self
	}

	// this function returns an ingestible job instance from a job report
	pub fn new_from_report(
		mut report: JobReport,
		stateful_job: SJob, // whichever type of job this should be is passed here
		next_jobs: Option<VecDeque<Box<dyn DynJob>>>,
	) -> Result<Box<dyn DynJob>, JobError> {
		Ok(Box::new(Self {
			id: report.id,
			state: rmp_serde::from_slice(
				&report
					.data
					.take()
					.ok_or_else(|| JobError::MissingJobDataState(report.id, report.name.clone()))?,
			)?,
			report: Some(report),
			stateful_job,
			next_jobs: next_jobs.unwrap_or_default(),
		}))
	}

	fn new_dependent(init: Init, parent_id: Uuid, parent_action: Option<String>) -> Box<Self> {
		let id = Uuid::new_v4();
		Box::new(Self {
			id,
			report: Some(JobReport::new_with_parent(
				id,
				SJob::NAME.to_string(),
				parent_id,
				parent_action,
			)),
			state: JobState {
				init,
				data: None,
				steps: VecDeque::new(),
				step_number: 0,
			},
			stateful_job: SJob::new(),
			next_jobs: VecDeque::new(),
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
		self.report()
			.as_ref()
			.expect("This method is using during queueing, so we still have a report")
			.id
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

	async fn run(
		&mut self,
		job_manager: Arc<JobManager>,
		ctx: &mut WorkerContext,
	) -> Result<(JobMetadata, JobRunErrors), JobError> {
		let mut job_should_run = true;
		let mut errors = vec![];
		info!(
			"Starting job {id} ({name})",
			id = self.id,
			name = self.name()
		);

		// Checking if we have a brand new job, or if we are resuming an old one.
		if self.state.data.is_none() {
			if let Err(e) = self.stateful_job.init(ctx, &mut self.state).await {
				match e {
					JobError::EarlyFinish { .. } => {
						info!("{e}");
						job_should_run = false;
					}
					JobError::StepCompletedWithErrors(errors_text) => errors.extend(errors_text),
					other => return Err(other),
				}
			}
		}

		let command_rx = ctx.command_rx.clone();
		let mut command_rx = command_rx.lock().await;

		// Run the job until it's done or we get a command
		while job_should_run && !self.state.steps.is_empty() {
			// Check for commands every iteration
			if let Ok(command) = command_rx.try_recv() {
				match command {
					WorkerCommand::Shutdown => {
						return Err(JobError::Paused(rmp_serde::to_vec_named(&self.state)?));
					}
					WorkerCommand::Cancel => {
						return Err(JobError::Canceled(rmp_serde::to_vec_named(&self.state)?));
					}
				}
			}

			let mut state_preserved = false;
			// Every X milliseconds, check the AtomicBool if we should pause or stay paused
			while ctx.paused.load(Ordering::Relaxed) {
				if !state_preserved {
					// Save the state of the job
					println!("Saving state {:?}", &self.report);
					// ctx.preserve_state(rmp_serde::to_vec_named(&self.state)?);
				}
				state_preserved = true;
				tokio::time::sleep(Duration::from_millis(500)).await;
			}

			// process job step and handle errors if any
			let step_result = self.stateful_job.execute_step(ctx, &mut self.state).await;
			match step_result {
				Err(JobError::EarlyFinish { .. }) => {
					step_result
						.map_err(|err| {
							warn!("{}", err);
						})
						.ok();
					break;
				}
				Err(JobError::StepCompletedWithErrors(errors_text)) => {
					warn!("Job<id='{}'> had a step with errors", self.id);
					errors.extend(errors_text);
				}
				maybe_err => maybe_err?,
			}
			// remove the step from the queue
			self.state.steps.pop_front();
			self.state.step_number += 1;
		}

		let metadata = self.stateful_job.finalize(ctx, &mut self.state).await?;

		let mut next_jobs = mem::take(&mut self.next_jobs);

		if let Some(mut next_job) = next_jobs.pop_front() {
			debug!(
				"Job '{}' requested to spawn '{}' now that it's complete!",
				self.name(),
				next_job.name()
			);
			next_job.set_next_jobs(next_jobs);

			if let Err(e) = job_manager.clone().ingest(&ctx.library, next_job).await {
				error!("Failed to ingest next job: {e}");
			}
		}

		Ok((metadata, errors))
	}

	fn hash(&self) -> u64 {
		<SJob::Init as JobInitData>::hash(&self.state.init)
	}

	fn set_next_jobs(&mut self, next_jobs: VecDeque<Box<dyn DynJob>>) {
		self.next_jobs = next_jobs;
	}

	fn serialize_state(&self) -> Result<Vec<u8>, JobError> {
		rmp_serde::to_vec_named(&self.state).map_err(Into::into)
	}

	async fn register_children(&mut self, library: &Library) -> Result<(), JobError> {
		for next_job in self.next_jobs.iter_mut() {
			if let Some(next_job_report) = next_job.report_mut() {
				if next_job_report.created_at.is_none() {
					next_job_report.create(library).await?
				}
			} else {
				return Err(JobError::MissingReport {
					id: next_job.id(),
					name: next_job.name().to_string(),
				});
			}
		}

		Ok(())
	}

	async fn pause_children(&mut self, library: &Library) -> Result<(), JobError> {
		for next_job in self.next_jobs.iter_mut() {
			let state = next_job.serialize_state()?;
			if let Some(next_job_report) = next_job.report_mut() {
				next_job_report.status = JobStatus::Paused;
				next_job_report.data = Some(state);
				next_job_report.update(library).await?;
			} else {
				return Err(JobError::MissingReport {
					id: next_job.id(),
					name: next_job.name().to_string(),
				});
			}
		}

		Ok(())
	}

	async fn cancel_children(&mut self, library: &Library) -> Result<(), JobError> {
		for next_job in self.next_jobs.iter_mut() {
			let state = next_job.serialize_state()?;
			if let Some(next_job_report) = next_job.report_mut() {
				next_job_report.status = JobStatus::Canceled;
				next_job_report.data = Some(state);
				next_job_report.update(library).await?;
			} else {
				return Err(JobError::MissingReport {
					id: next_job.id(),
					name: next_job.name().to_string(),
				});
			}
		}

		Ok(())
	}
}

#[macro_export]
macro_rules! extract_job_data {
	($state:ident) => {{
		$state
			.data
			.as_ref()
			.expect("critical error: missing data on job state")
	}};
}

#[macro_export]
macro_rules! extract_job_data_mut {
	($state:ident) => {{
		$state
			.data
			.as_mut()
			.expect("critical error: missing data on job state")
	}};
}
