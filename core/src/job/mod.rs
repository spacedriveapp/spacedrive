use crate::library::Library;

use std::{
	collections::{hash_map::DefaultHasher, VecDeque},
	fmt,
	hash::{Hash, Hasher},
	mem,
	sync::Arc,
	time::Instant,
};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tokio::{select, sync::mpsc};
use tracing::{debug, info, warn};
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

#[derive(Debug, Default)]
pub struct JobRunErrors(pub Vec<String>);

impl From<Vec<String>> for JobRunErrors {
	fn from(errors: Vec<String>) -> Self {
		Self(errors)
	}
}

pub struct JobRunOutput {
	pub metadata: JobMetadata,
	pub errors: JobRunErrors,
	pub next_job: Option<Box<dyn DynJob>>,
}

/// `JobInitData` is a trait to represent the data being passed to initialize a `Job`
#[deprecated]
pub trait JobInitData: Serialize + DeserializeOwned + Send + Sync + Hash + fmt::Debug {
	type Job: StatefulJob;

	fn hash(&self) -> u64 {
		let mut s = DefaultHasher::new();
		<Self::Job as StatefulJob>::NAME.hash(&mut s);
		<Self as Hash>::hash(self, &mut s);
		s.finish()
	}
}

pub trait JobRunMetadata:
	Default + Serialize + DeserializeOwned + Send + Sync + fmt::Debug
{
	fn update(&mut self, new_data: Self);
}

impl JobRunMetadata for () {
	fn update(&mut self, _new_data: Self) {}
}

#[async_trait::async_trait]
pub trait StatefulJob:
	Serialize + DeserializeOwned + Hash + fmt::Debug + Send + Sync + Sized + 'static
{
	type Data: Serialize + DeserializeOwned + Send + Sync + fmt::Debug;
	type Step: Serialize + DeserializeOwned + Send + Sync + fmt::Debug;
	type RunMetadata: JobRunMetadata;

	/// The name of the job is a unique human readable identifier for the job.
	const NAME: &'static str;
	const IS_BACKGROUND: bool = false;

	/// Construct a new instance of the job. This is used so the user can pass `Self::Init` into the `spawn_job` function and we can still run the job.
	/// This does remove the flexibility of being able to pass arguments into the job's struct but with resumable jobs I view that as an anti-pattern anyway.
	fn new() -> Self {
		todo!();
	}

	/// initialize the steps for the job
	async fn init(
		&self,
		ctx: &WorkerContext,
		// init: &Self::Init,
		data: &mut Option<Self::Data>,
	) -> Result<JobInitOutput<Self::RunMetadata, Self::Step>, JobError>;

	/// is called for each step in the job. These steps are created in the `Self::init` method.
	async fn execute_step(
		&self,
		ctx: &WorkerContext,
		step: CurrentStep<'_, Self::Step>,
		data: &Self::Data,
		run_metadata: &Self::RunMetadata,
	) -> Result<JobStepOutput<Self::Step, Self::RunMetadata>, JobError>;

	/// is called after all steps have been executed
	async fn finalize(
		&self,
		ctx: &WorkerContext,
		data: &Option<Self::Data>,
		run_metadata: &Self::RunMetadata,
	) -> JobResult;
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
		ctx: WorkerContext,
		commands_rx: mpsc::Receiver<WorkerCommand>,
	) -> Result<JobRunOutput, JobError>;
	fn hash(&self) -> u64;
	fn set_next_jobs(&mut self, next_jobs: VecDeque<Box<dyn DynJob>>);
	fn serialize_state(&self) -> Result<Vec<u8>, JobError>;
	async fn register_children(&mut self, library: &Library) -> Result<(), JobError>;
	async fn pause_children(&mut self, library: &Library) -> Result<(), JobError>;
	async fn cancel_children(&mut self, library: &Library) -> Result<(), JobError>;
}

pub struct Job<SJob: StatefulJob> {
	id: Uuid,
	hash: u64,
	report: Option<JobReport>,
	state: Option<JobState<SJob>>,
	stateful_job: Option<SJob>,
	next_jobs: VecDeque<Box<dyn DynJob>>,
}

#[deprecated]
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
			hash: <SJob::Init as JobInitData>::hash(&init),
			report: Some(JobReport::new(id, SJob::NAME.to_string())),
			state: Some(JobState {
				init,
				data: None,
				steps: VecDeque::new(),
				step_number: 0,
				run_metadata: Default::default(),
			}),
			stateful_job: Some(SJob::new()),
			next_jobs: VecDeque::new(),
		})
	}

	pub fn new_with_action(init: Init, action: impl AsRef<str>) -> Box<Self> {
		let id = Uuid::new_v4();
		Box::new(Self {
			id,
			hash: <SJob::Init as JobInitData>::hash(&init),
			report: Some(JobReport::new_with_action(
				id,
				SJob::NAME.to_string(),
				action,
			)),
			state: Some(JobState {
				init,
				data: None,
				steps: VecDeque::new(),
				step_number: 0,
				run_metadata: Default::default(),
			}),
			stateful_job: Some(SJob::new()),
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
		let state = rmp_serde::from_slice::<JobState<_>>(
			&report
				.data
				.take()
				.ok_or_else(|| JobError::MissingJobDataState(report.id, report.name.clone()))?,
		)?;

		Ok(Box::new(Self {
			id: report.id,
			hash: <SJob::Init as JobInitData>::hash(&state.init),
			state: Some(state),
			report: Some(report),
			stateful_job: Some(stateful_job),
			next_jobs: next_jobs.unwrap_or_default(),
		}))
	}

	fn new_dependent(init: Init, parent_id: Uuid, parent_action: Option<String>) -> Box<Self> {
		let id = Uuid::new_v4();
		Box::new(Self {
			id,
			hash: <SJob::Init as JobInitData>::hash(&init),
			report: Some(JobReport::new_with_parent(
				id,
				SJob::NAME.to_string(),
				parent_id,
				parent_action,
			)),
			state: Some(JobState {
				init,
				data: None,
				steps: VecDeque::new(),
				step_number: 0,
				run_metadata: Default::default(),
			}),
			stateful_job: Some(SJob::new()),
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
	pub run_metadata: Job::RunMetadata,
}

pub struct JobInitOutput<RunMetadata, Step> {
	run_metadata: RunMetadata,
	steps: VecDeque<Step>,
	errors: JobRunErrors,
}

impl<RunMetadata, Step> From<(RunMetadata, Vec<Step>)> for JobInitOutput<RunMetadata, Step> {
	fn from((run_metadata, steps): (RunMetadata, Vec<Step>)) -> Self {
		Self {
			run_metadata,
			steps: VecDeque::from(steps),
			errors: Default::default(),
		}
	}
}

impl<Step> From<Vec<Step>> for JobInitOutput<(), Step> {
	fn from(steps: Vec<Step>) -> Self {
		Self {
			run_metadata: (),
			steps: VecDeque::from(steps),
			errors: Default::default(),
		}
	}
}

impl<RunMetadata, Step> From<(RunMetadata, Vec<Step>, JobRunErrors)>
	for JobInitOutput<RunMetadata, Step>
{
	fn from((run_metadata, steps, errors): (RunMetadata, Vec<Step>, JobRunErrors)) -> Self {
		Self {
			run_metadata,
			steps: VecDeque::from(steps),
			errors,
		}
	}
}

pub struct CurrentStep<'step, Step> {
	pub step: &'step Step,
	pub step_number: usize,
}

pub struct JobStepOutput<Step, RunMetadata> {
	maybe_more_steps: Option<Vec<Step>>,
	maybe_more_metadata: Option<RunMetadata>,
	errors: JobRunErrors,
}

impl<Step, RunMetadata: JobRunMetadata> From<Vec<Step>> for JobStepOutput<Step, RunMetadata> {
	fn from(more_steps: Vec<Step>) -> Self {
		Self {
			maybe_more_steps: Some(more_steps),
			maybe_more_metadata: None,
			errors: Default::default(),
		}
	}
}

impl<Step, RunMetadata: JobRunMetadata> From<RunMetadata> for JobStepOutput<Step, RunMetadata> {
	fn from(more_metadata: RunMetadata) -> Self {
		Self {
			maybe_more_steps: None,
			maybe_more_metadata: Some(more_metadata),
			errors: Default::default(),
		}
	}
}

impl<Step, RunMetadata: JobRunMetadata> From<JobRunErrors> for JobStepOutput<Step, RunMetadata> {
	fn from(errors: JobRunErrors) -> Self {
		Self {
			maybe_more_steps: None,
			maybe_more_metadata: None,
			errors,
		}
	}
}

impl<Step, RunMetadata: JobRunMetadata> From<(Vec<Step>, RunMetadata)>
	for JobStepOutput<Step, RunMetadata>
{
	fn from((more_steps, more_metadata): (Vec<Step>, RunMetadata)) -> Self {
		Self {
			maybe_more_steps: Some(more_steps),
			maybe_more_metadata: Some(more_metadata),
			errors: Default::default(),
		}
	}
}

impl<Step, RunMetadata: JobRunMetadata> From<(Vec<Step>, RunMetadata, JobRunErrors)>
	for JobStepOutput<Step, RunMetadata>
{
	fn from((more_steps, more_metadata, errors): (Vec<Step>, RunMetadata, JobRunErrors)) -> Self {
		Self {
			maybe_more_steps: Some(more_steps),
			maybe_more_metadata: Some(more_metadata),
			errors,
		}
	}
}

impl<Step, RunMetadata: JobRunMetadata> From<Option<()>> for JobStepOutput<Step, RunMetadata> {
	fn from(_: Option<()>) -> Self {
		Self {
			maybe_more_steps: None,
			maybe_more_metadata: None,
			errors: Vec::new().into(),
		}
	}
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
		ctx: WorkerContext,
		mut commands_rx: mpsc::Receiver<WorkerCommand>,
	) -> Result<JobRunOutput, JobError> {
		let job_name = self.name();
		let job_id = self.id;
		let mut errors = vec![];
		info!("Starting Job <id='{job_id}', name='{job_name}'>");

		let JobState {
			init,
			data,
			mut steps,
			mut step_number,
			mut run_metadata,
		} = self
			.state
			.take()
			.expect("critical error: missing job state");

		let stateful_job = Arc::new(
			self.stateful_job
				.take()
				.expect("critical error: missing stateful job"),
		);

		let ctx = Arc::new(ctx);
		let init_arc = Arc::new(init);

		let mut job_should_run = true;
		let job_time = Instant::now();

		// Checking if we have a brand new job, or if we are resuming an old one.
		let working_data = if let Some(data) = data {
			Some(data)
		} else {
			// Job init phase
			let inner_ctx = Arc::clone(&ctx);
			let inner_init = Arc::clone(&init_arc);
			let inner_stateful_job = Arc::clone(&stateful_job);

			let init_time = Instant::now();

			let mut init_handle = tokio::spawn(async move {
				let mut new_data = None;
				let res = inner_stateful_job
					.init(&inner_ctx, &inner_init, &mut new_data)
					.await;

				if let Ok(res) = res.as_ref() {
					inner_ctx.progress(vec![JobReportUpdate::TaskCount(res.steps.len())]);
				}

				(new_data, res)
			});

			loop {
				select! {
					Some(command) = commands_rx.recv() => {
						match command {
							WorkerCommand::Pause(when) => {
								debug!(
									"Pausing Job at init phase <id='{job_id}', name='{job_name}'> took {:?}",
									when.elapsed()
								);

								// In case of a Pause command, we keep waiting for the next command
								let paused_time = Instant::now();
								while let Some(command) = commands_rx.recv().await {
									match command {
										WorkerCommand::Resume(when) => {
											debug!(
												"Resuming Job at init phase <id='{job_id}', name='{job_name}'> took {:?}",
												when.elapsed()
											);
											debug!(
												"Total paused time {:?} Job <id='{job_id}', name='{job_name}'>",
												paused_time.elapsed()
											);
											break;
										}
										// The job can also be shutdown or canceled while paused
										WorkerCommand::Shutdown(when, signal_tx) => {
											init_handle.abort();

											debug!(
												"Shuting down Job at init phase <id='{job_id}', name='{job_name}'> \
												 took {:?} after running for {:?}",
												when.elapsed(),
												init_time.elapsed(),
											);
											debug!("Total paused time {:?}", paused_time.elapsed());

											// Shutting down at init phase will abort the job
											return Err(
												JobError::Canceled(signal_tx)
											);
										}
										WorkerCommand::Cancel(when, signal_tx) => {
											init_handle.abort();
											debug!(
												"Canceling Job at init phase <id='{job_id}', name='{job_name}'> \
												 took {:?} after running for {:?}",
												when.elapsed(),
												init_time.elapsed(),
											);
											debug!(
												"Total paused time {:?} Job <id='{job_id}', name='{job_name}'>",
												paused_time.elapsed()
											);
											return Err(JobError::Canceled(signal_tx));
										}
										WorkerCommand::Pause(_) => {
											// We continue paused lol
										}
									}
								}
							}

							WorkerCommand::Resume(_) => {
								// We're already running so we just ignore this command
							}

							WorkerCommand::Shutdown(when, signal_tx) => {
								init_handle.abort();

								debug!(
									"Shuting down Job at init phase <id='{job_id}', name='{job_name}'> took {:?} \
									 after running for {:?}",
									when.elapsed(),
									init_time.elapsed(),
								);

								// Shutting down at init phase will abort the job
								return Err(
									JobError::Canceled(signal_tx)
								);
							}
							WorkerCommand::Cancel(when, signal_tx) => {
								init_handle.abort();
								debug!(
									"Canceling Job at init phase <id='{job_id}', name='{job_name}'> took {:?} \
									 after running for {:?}",
									when.elapsed(),
									init_time.elapsed()
								);
								return Err(JobError::Canceled(signal_tx));
							}
						}
					}
					init_res = &mut init_handle => {
						let (new_data, res) = init_res?;
						debug!("Init phase took {:?} Job <id='{job_id}', name='{job_name}'>", init_time.elapsed());

						match res {
							Ok(JobInitOutput {
								run_metadata: new_run_metadata,
								steps: new_steps,
								errors: JobRunErrors(new_errors),
							}) => {
								steps = new_steps;
								errors.extend(new_errors);
								run_metadata.update(new_run_metadata);
							}

							Err(e) if matches!(e, JobError::EarlyFinish { .. }) => {
								job_should_run = false;
								info!("{e}");
							}
							Err(other) => return Err(other),
						}

						break new_data;
					}
				}
			}
		};

		// Run the job until it's done or we get a command
		let data = if let Some(working_data) = working_data {
			let working_data_arc = Arc::new(working_data);

			// Job run phase
			while job_should_run && !steps.is_empty() {
				let steps_len = steps.len();

				let run_metadata_arc = Arc::new(run_metadata);
				let step_arc =
					Arc::new(steps.pop_front().expect("just checked that we have steps"));

				// Need these bunch of Arcs to be able to move them into the async block of tokio::spawn
				let inner_ctx = Arc::clone(&ctx);
				let inner_init = Arc::clone(&init_arc);
				let inner_run_metadata = Arc::clone(&run_metadata_arc);
				let inner_working_data = Arc::clone(&working_data_arc);
				let inner_step = Arc::clone(&step_arc);
				let inner_stateful_job = Arc::clone(&stateful_job);

				let step_time = Instant::now();

				let mut job_step_handle = tokio::spawn(async move {
					inner_stateful_job
						.execute_step(
							&inner_ctx,
							&inner_init,
							CurrentStep {
								step: &inner_step,
								step_number,
							},
							&inner_working_data,
							&inner_run_metadata,
						)
						.await
				});

				loop {
					select! {
						// Here we have a channel that we use to receive commands from the worker
						Some(command) = commands_rx.recv() => {
							match command {
								WorkerCommand::Pause(when) => {
									debug!(
										"Pausing Job <id='{job_id}', name='{job_name}'> took {:?}",
										when.elapsed()
									);

									// In case of a Pause command, we keep waiting for the next command
									let paused_time = Instant::now();
									while let Some(command) = commands_rx.recv().await {
										match command {
											WorkerCommand::Resume(when) => {
												debug!(
													"Resuming Job <id='{job_id}', name='{job_name}'> took {:?}",
													when.elapsed(),
												);
												debug!(
													"Total paused time {:?} Job <id='{job_id}', name='{job_name}'>",
													paused_time.elapsed(),
												);
												break;
											}
											// The job can also be shutdown or canceled while paused
											WorkerCommand::Shutdown(when, signal_tx) => {
												job_step_handle.abort();

												debug!(
													"Shuting down Job <id='{job_id}', name='{job_name}'> took {:?} \
													 after running for {:?}",
													when.elapsed(),
													job_time.elapsed(),
												);
												debug!(
													"Total paused time {:?} Job <id='{job_id}', name='{job_name}'>",
													paused_time.elapsed(),
												);

												// Taking back the last step, so it can run to completion later
												steps.push_front(
													Arc::try_unwrap(step_arc)
														.expect("step already ran, no more refs"),
												);

												return Err(
													JobError::Paused(
														rmp_serde::to_vec_named(
															&JobState::<SJob> {
																init: Arc::try_unwrap(init_arc)
																	.expect("handle abort already ran, no more refs"),
																data: Some(
																	Arc::try_unwrap(working_data_arc)
																		.expect("handle abort already ran, no more refs"),
																),
																steps,
																step_number,
																run_metadata: Arc::try_unwrap(run_metadata_arc)
																	.expect("handle abort already ran, no more refs"),
															}
														)?,
														signal_tx
													)
												);
											}
											WorkerCommand::Cancel(when, signal_tx) => {
												job_step_handle.abort();
												debug!(
													"Canceling Job <id='{job_id}', name='{job_name}'> \
													 took {:?} after running for {:?}",
													when.elapsed(),
													job_time.elapsed(),
												);
												debug!(
													"Total paused time {:?} Job <id='{job_id}', name='{job_name}'>",
													paused_time.elapsed(),
												);
												return Err(JobError::Canceled(signal_tx));
											}
											WorkerCommand::Pause(_) => {
												// We continue paused lol
											}
										}
									}
								}
								WorkerCommand::Resume(_) => {
									// We're already running so we just ignore this command
								}

								WorkerCommand::Shutdown(when, signal_tx) => {
									job_step_handle.abort();

									debug!(
										"Shuting down Job <id='{job_id}', name='{job_name}'> took {:?} \
										 after running for {:?}",
										when.elapsed(),
										job_time.elapsed(),
									);

									// Taking back the last step, so it can run to completion later
									steps.push_front(
										Arc::try_unwrap(step_arc)
											.expect("handle abort already ran, no more refs"),
									);

									return Err(
										JobError::Paused(
											rmp_serde::to_vec_named(
												&JobState::<SJob> {
													init: Arc::try_unwrap(init_arc)
														.expect("handle abort already ran, no more refs"),
													data: Some(
														Arc::try_unwrap(working_data_arc)
															.expect("handle abort already ran, no more refs"),
													),
													steps,
													step_number,
													run_metadata: Arc::try_unwrap(run_metadata_arc)
														.expect("step already ran, no more refs"),
												}
											)?,
											signal_tx
										)
									);
								}
								WorkerCommand::Cancel(when, signal_tx) => {
									job_step_handle.abort();
									debug!(
										"Canceling Job <id='{job_id}', name='{job_name}'> took {:?} \
										 after running for {:?}",
										when.elapsed(),
										job_time.elapsed(),
									);
									return Err(JobError::Canceled(signal_tx));
								}
							}
						}

						// Here we actually run the job, step by step
						step_result = &mut job_step_handle => {
							debug!(
								"Step finished in {:?} Job <id='{job_id}', name='{job_name}'>",
								step_time.elapsed(),
							);

							run_metadata = Arc::try_unwrap(run_metadata_arc)
								.expect("step already ran, no more refs");

							match step_result? {
								Ok(JobStepOutput {
									maybe_more_steps,
									maybe_more_metadata,
									errors: JobRunErrors(new_errors)
								}) => {
									let mut events = vec![
										JobReportUpdate::CompletedTaskCount(
											step_number + 1,
										)
									];

									if let Some(more_steps) = maybe_more_steps {
										events.push(JobReportUpdate::TaskCount(
											steps_len + more_steps.len(),
										));

										steps.extend(more_steps);
									}

									if let Some(more_metadata) = maybe_more_metadata {
										run_metadata.update(more_metadata);
									}

									ctx.progress(events);

									if !new_errors.is_empty() {
										warn!("Job<id='{job_id}', name='{job_name}'> had a step with errors");
										errors.extend(new_errors);
									}
								}
								Err(e) if matches!(e, JobError::EarlyFinish { .. }) => {
									info!("{e}");
									break;
								}
								Err(e) => return Err(e),
							}
							// remove the step from the queue
							step_number += 1;

							break;
						}
					}
				}
			}

			debug!(
				"Total job run time {:?} Job <id='{job_id}', name='{job_name}'>",
				job_time.elapsed()
			);

			Some(Arc::try_unwrap(working_data_arc).expect("job already ran, no more refs"))
		} else {
			warn!("Tried to run a job without data Job <id='{job_id}', name='{job_name}'>");
			None
		};

		let state = JobState::<SJob> {
			init: Arc::try_unwrap(init_arc).expect("job already ran, no more refs"),
			data,
			steps,
			step_number,
			run_metadata,
		};

		let metadata = stateful_job
			.finalize(&ctx, &state.data, &state.run_metadata, &state.init)
			.await?;

		let mut next_jobs = mem::take(&mut self.next_jobs);

		Ok(JobRunOutput {
			metadata,
			errors: errors.into(),
			next_job: next_jobs.pop_front().map(|mut next_job| {
				debug!(
					"Job<id='{job_id}', name='{job_name}'> requesting to spawn '{}' now that it's complete!",
					next_job.name()
				);
				next_job.set_next_jobs(next_jobs);

				next_job
			}),
		})
	}

	fn hash(&self) -> u64 {
		self.hash
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
