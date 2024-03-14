use crate::{jobs::JobId, Error};

use futures::{stream, StreamExt};
use futures_concurrency::stream::Merge;
use sd_task_system::{IntoTask, Task, TaskHandle, TaskRemoteController, TaskSystemError};
use tokio::spawn;
use tokio::task::JoinHandle;
use tracing::warn;

use std::collections::VecDeque;
use std::hash::Hash;
use std::pin::pin;

use async_channel as chan;
use futures_concurrency::future::{Join, TryJoin};
use serde::Serialize;
use specta::Type;

use super::report::{Report, ReportBuilder};
use super::Command;

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

pub(crate) enum ReturnStatus {
	Completed(JobOutput),
	Failed(Error),
	Shutdown(Vec<Box<dyn Task<Error>>>),
	Canceled,
}

#[async_trait::async_trait]
pub(crate) trait Job: Send + Hash + 'static {
	const NAME: &'static str;

	async fn run(mut self, dispatcher: TaskDispatcher) -> ReturnStatus;
}

pub(crate) trait IntoJob<J: Job> {
	fn into_job(self) -> JobHolder<J>;
}

impl<J: Job> IntoJob<J> for J {
	fn into_job(self) -> JobHolder<J> {
		let id = JobId::new_v4();

		JobHolder {
			id,
			job: self,
			report: ReportBuilder::new(id, J::NAME.to_string()).build(),
			next_jobs: VecDeque::new(),
		}
	}
}

#[derive(Serialize, Type)]
pub struct JobOutput {
	id: JobId,
	job_type: String,
	data: JobOutputData,
	non_critical_errors: Vec<rspc::Error>,
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

	pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
		self.report_builder = self.report_builder.with_metadata(metadata);
		self
	}
}

pub struct JobHolder<J: Job> {
	id: JobId,
	job: J,
	next_jobs: VecDeque<Box<dyn DynJob>>,
	report: Report,
}

struct JobHandle {
	id: JobId,
	next_jobs: VecDeque<Box<dyn DynJob>>,
	report: Report,
	commands_tx: chan::Sender<Command>,
}

trait DynJob: Send + 'static {
	fn dispatch(
		self,
		dispatcher: sd_task_system::TaskDispatcher<Error>,
		done_tx: chan::Sender<(JobId, ReturnStatus)>,
	) -> JobHandle;
}

impl<J: Job> DynJob for JobHolder<J> {
	fn dispatch(
		self,
		dispatcher: sd_task_system::TaskDispatcher<Error>,
		done_tx: chan::Sender<(JobId, ReturnStatus)>,
	) -> JobHandle {
		let (commands_tx, commands_rx) = chan::bounded(8);

		let JobHolder {
			id,
			job,
			report,
			next_jobs,
		} = self;

		spawn(to_spawn_job(id, job, dispatcher, commands_rx, done_tx));

		JobHandle {
			id,
			next_jobs,
			report,
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
							.map(|controller| controller.pause())
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
							.map(|controller| controller.resume())
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
							.map(|controller| controller.cancel())
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
