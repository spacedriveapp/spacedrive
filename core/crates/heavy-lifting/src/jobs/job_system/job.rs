use crate::{jobs::JobId, Error};

use sd_prisma::prisma::PrismaClient;
use sd_task_system::{IntoTask, Task, TaskHandle, TaskRemoteController, TaskSystemError};

use std::{
	collections::VecDeque,
	hash::{DefaultHasher, Hash, Hasher},
	pin::pin,
};

use async_channel as chan;
use chrono::{DateTime, Utc};
use futures::{stream, StreamExt};
use futures_concurrency::{
	future::{Join, TryJoin},
	stream::Merge,
};
use serde::Serialize;
use specta::Type;
use tokio::spawn;
use tracing::warn;

use super::{
	report::{Report, ReportBuilder, ReportInputMetadata, ReportOutputMetadata, Status},
	Command, JobSystemError,
};

pub struct TaskDispatcher {
	dispatcher: sd_task_system::TaskDispatcher<Error>,
	remote_controllers_tx: chan::Sender<TaskRemoteController>,
}

impl TaskDispatcher {
	fn new(
		dispatcher: sd_task_system::TaskDispatcher<Error>,
	) -> (Self, chan::Receiver<TaskRemoteController>) {
		let (remote_controllers_tx, remote_controllers_rx) = chan::unbounded();

		(
			Self {
				dispatcher,
				remote_controllers_tx,
			},
			remote_controllers_rx,
		)
	}

	pub async fn dispatch(&self, into_task: impl IntoTask<Error>) -> TaskHandle<Error> {
		let handle = self.dispatcher.dispatch(into_task).await;

		self.remote_controllers_tx
			.send(handle.remote_controller())
			.await
			.expect("remote controllers tx closed");

		handle
	}

	pub async fn dispatch_many(
		&self,
		into_tasks: Vec<impl IntoTask<Error>>,
	) -> Vec<TaskHandle<Error>> {
		let handles = self.dispatcher.dispatch_many(into_tasks).await;

		for handle in &handles {
			self.remote_controllers_tx
				.send(handle.remote_controller())
				.await
				.expect("remote controllers tx closed");
		}

		handles
			.iter()
			.map(|handle| self.remote_controllers_tx.send(handle.remote_controller()))
			.collect::<Vec<_>>()
			.try_join()
			.await
			.expect("remote controllers tx closed");

		handles
	}
}

pub enum ReturnStatus {
	Completed(JobOutput),
	Failed(Error),
	Shutdown(Vec<Box<dyn Task<Error>>>),
	Canceled,
}

#[async_trait::async_trait]
pub trait Job: Send + Sync + Hash + 'static {
	const NAME: &'static str;

	async fn run(mut self, dispatcher: TaskDispatcher) -> ReturnStatus;
}

pub trait IntoJob<J: Job> {
	fn into_job(self) -> Box<dyn DynJob>;
}

impl<J: Job> IntoJob<J> for J {
	fn into_job(self) -> Box<dyn DynJob> {
		let id = JobId::new_v4();

		Box::new(JobHolder {
			id,
			job: self,
			report: ReportBuilder::new(id, J::NAME.to_string()).build(),
			next_jobs: VecDeque::new(),
		})
	}
}

impl<J: Job> IntoJob<J> for JobBuilder<J> {
	fn into_job(self) -> Box<dyn DynJob> {
		self.build()
	}
}

#[derive(Serialize, Type)]
pub struct JobOutput {
	pub id: JobId,
	pub job_type: String,
	pub data: JobOutputData,
	pub metadata: ReportOutputMetadata,
	pub non_critical_errors: Vec<rspc::Error>,
}

#[derive(Serialize, Type)]
pub enum JobOutputData {
	Empty,
	// TODO: Add more types
}

pub struct JobBuilder<J: Job> {
	id: JobId,
	job: J,
	report_builder: ReportBuilder,
	next_jobs: VecDeque<Box<dyn DynJob>>,
}

impl<J: Job> JobBuilder<J> {
	pub fn build(self) -> Box<JobHolder<J>> {
		Box::new(JobHolder::<J> {
			id: self.id,
			job: self.job,
			report: self.report_builder.build(),
			next_jobs: VecDeque::new(),
		})
	}

	pub fn new(job: J) -> Self {
		let id = JobId::new_v4();
		Self {
			id,
			job,
			report_builder: ReportBuilder::new(id, J::NAME.to_string()),
			next_jobs: VecDeque::new(),
		}
	}

	pub fn with_action(mut self, action: impl Into<String>) -> Self {
		self.report_builder = self.report_builder.with_action(action);
		self
	}

	pub fn with_parent_id(mut self, parent_id: JobId) -> Self {
		self.report_builder = self.report_builder.with_parent_id(parent_id);
		self
	}

	pub fn with_metadata(mut self, metadata: ReportInputMetadata) -> Self {
		self.report_builder = self.report_builder.with_metadata(metadata);
		self
	}

	pub fn enqueue_next(mut self, next: impl Job) -> Self {
		let next_job_order = self.next_jobs.len() + 1;

		let mut child_job_builder = JobBuilder::new(next).with_parent_id(self.id);

		if let Some(parent_action) = &self.report_builder.action {
			child_job_builder =
				child_job_builder.with_action(format!("{parent_action}-{next_job_order}"));
		}

		self.next_jobs.push_back(child_job_builder.build());

		self
	}
}

pub struct JobHolder<J: Job> {
	pub(super) id: JobId,
	job: J,
	next_jobs: VecDeque<Box<dyn DynJob>>,
	report: Report,
}

pub struct JobHandle {
	pub(crate) next_jobs: VecDeque<Box<dyn DynJob>>,
	pub(crate) report: Report,
	pub(crate) commands_tx: chan::Sender<Command>,
}

impl JobHandle {
	pub async fn send_command(
		&mut self,
		command: Command,
		db: &PrismaClient,
	) -> Result<(), JobSystemError> {
		if self.commands_tx.send(command).await.is_err() {
			warn!("Tried to send a {command:?} to a job that was already completed");

			Ok(())
		} else {
			let new_status = match command {
				Command::Pause => Status::Paused,
				Command::Resume => return Ok(()),
				Command::Cancel => Status::Canceled,
			};

			self.next_jobs
				.iter_mut()
				.map(|dyn_job| dyn_job.report_mut())
				.map(|next_job_report| async {
					next_job_report.status = new_status;
					next_job_report.update(db).await
				})
				.collect::<Vec<_>>()
				.try_join()
				.await
				.map(|_| ())
				.map_err(Into::into)
		}
	}

	pub async fn register_start(
		&mut self,
		start_time: DateTime<Utc>,
		db: &PrismaClient,
	) -> Result<(), JobSystemError> {
		let Self {
			next_jobs, report, ..
		} = self;

		report.status = Status::Running;
		if report.started_at.is_none() {
			report.started_at = Some(start_time);
		}

		// If the report doesn't have a created_at date, it's a new report
		if report.created_at.is_none() {
			report.create(db).await?;
		} else {
			// Otherwise it can be a job being resumed or a children job that was already been created
			report.update(db).await?;
		}

		// Registering children jobs
		next_jobs
			.iter_mut()
			.map(|dyn_job| dyn_job.report_mut())
			.map(|next_job_report| async {
				if next_job_report.created_at.is_none() {
					next_job_report.create(db).await
				} else {
					Ok(())
				}
			})
			.collect::<Vec<_>>()
			.try_join()
			.await
			.map(|_| ())
			.map_err(Into::into)
	}

	pub async fn complete(
		&mut self,
		output: &JobOutput,
		db: &PrismaClient,
	) -> Result<(), JobSystemError> {
		let Self { report, .. } = self;

		let status = if output.non_critical_errors.is_empty() {
			Status::Completed
		} else {
			Status::CompletedWithErrors
		};

		// TODO: Update the report with the output data
		// report.metadata

		Ok(())
	}
}

pub trait DynJob: Send + Sync + 'static {
	fn id(&self) -> JobId;

	fn job_name(&self) -> &'static str;

	fn hash(&self) -> u64;

	fn report_mut(&mut self) -> &mut Report;

	fn dispatch(
		self: Box<Self>,
		dispatcher: sd_task_system::TaskDispatcher<Error>,
		done_tx: chan::Sender<(JobId, ReturnStatus)>,
	) -> JobHandle;
}

impl<J: Job> DynJob for JobHolder<J> {
	fn id(&self) -> JobId {
		self.id
	}

	fn job_name(&self) -> &'static str {
		J::NAME
	}

	fn hash(&self) -> u64 {
		let mut hasher = DefaultHasher::new();
		J::NAME.hash(&mut hasher);
		self.job.hash(&mut hasher);
		hasher.finish()
	}

	fn report_mut(&mut self) -> &mut Report {
		&mut self.report
	}

	fn dispatch(
		self: Box<Self>,
		dispatcher: sd_task_system::TaskDispatcher<Error>,
		done_tx: chan::Sender<(JobId, ReturnStatus)>,
	) -> JobHandle {
		let (commands_tx, commands_rx) = chan::bounded(8);

		spawn(to_spawn_job(
			self.id,
			self.job,
			dispatcher,
			commands_rx,
			done_tx,
		));

		JobHandle {
			next_jobs: self.next_jobs,
			report: self.report,
			commands_tx,
		}
	}
}

async fn to_spawn_job(
	id: JobId,
	job: impl Job,
	dispatcher: sd_task_system::TaskDispatcher<Error>,
	commands_rx: chan::Receiver<Command>,
	done_tx: chan::Sender<(JobId, ReturnStatus)>,
) {
	enum StreamMessage {
		Commands(Command),
		NewRemoteController(TaskRemoteController),
		Done(ReturnStatus),
	}

	let mut remote_controllers = vec![];

	let (dispatcher, remote_controllers_rx) = TaskDispatcher::new(dispatcher);

	let mut msgs_stream = pin!((
		commands_rx.map(StreamMessage::Commands),
		remote_controllers_rx.map(StreamMessage::NewRemoteController),
		stream::once(job.run(dispatcher)).map(StreamMessage::Done),
	)
		.merge());

	while let Some(msg) = msgs_stream.next().await {
		match msg {
			StreamMessage::NewRemoteController(remote_controller) => {
				remote_controllers.push(remote_controller);
			}
			StreamMessage::Commands(command) => {
				remote_controllers.retain(|controller| !controller.is_done());

				match command {
					Command::Pause => {
						remote_controllers
							.iter()
							.map(TaskRemoteController::pause)
							.collect::<Vec<_>>()
							.join()
							.await
							.into_iter()
							.for_each(|res| {
								if let Err(e) = res {
									assert!(matches!(e, TaskSystemError::TaskNotFound(_)));

									warn!("Tried to pause a task that was already completed");
								}
							});
					}
					Command::Resume => {
						remote_controllers
							.iter()
							.map(TaskRemoteController::resume)
							.collect::<Vec<_>>()
							.join()
							.await
							.into_iter()
							.for_each(|res| {
								if let Err(e) = res {
									assert!(matches!(e, TaskSystemError::TaskNotFound(_)));

									warn!("Tried to pause a task that was already completed");
								}
							});
					}
					Command::Cancel => {
						remote_controllers
							.iter()
							.map(TaskRemoteController::cancel)
							.collect::<Vec<_>>()
							.join()
							.await;

						return done_tx
							.send((id, ReturnStatus::Canceled))
							.await
							.expect("jobs done tx closed");
					}
				}
			}

			StreamMessage::Done(res) => {
				#[cfg(debug_assertions)]
				{
					// Just a sanity check to make sure we don't have any pending tasks left
					remote_controllers.retain(|controller| !controller.is_done());
					assert!(remote_controllers.is_empty());
					// Using #[cfg(debug_assertions)] to don't pay this retain cost in release builds
				}

				return done_tx.send((id, res)).await.expect("jobs done tx closed");
			}
		}
	}
}
