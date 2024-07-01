use sd_task_system::{
	BaseTaskDispatcher, ExecStatus, Interrupter, Task, TaskDispatcher, TaskHandle, TaskId,
	TaskOutput, TaskStatus,
};

use std::{
	path::{Path, PathBuf},
	sync::Arc,
	time::Duration,
};

use async_channel as chan;
use async_trait::async_trait;
use futures::stream::{self, FuturesUnordered, StreamExt};
use futures_concurrency::future::Race;
use serde::{Deserialize, Serialize};
use tokio::{fs, spawn, sync::broadcast};
use tracing::{error, info, trace, warn};

use crate::common::tasks::TimedTaskOutput;

use super::tasks::{SampleError, TimeTask};

const SAMPLE_ACTOR_SAVE_STATE_FILE_NAME: &str = "sample_actor_save_state.bin";

pub struct SampleActor {
	data: Arc<String>, // Can hold any kind of actor data, like an AI model
	task_dispatcher: BaseTaskDispatcher<SampleError>,
	task_handles_tx: chan::Sender<TaskHandle<SampleError>>,
}

impl SampleActor {
	pub async fn new(
		data_directory: impl AsRef<Path>,
		data: String,
		task_dispatcher: BaseTaskDispatcher<SampleError>,
	) -> (Self, broadcast::Receiver<()>) {
		let (task_handles_tx, task_handles_rx) = chan::bounded(8);

		let (idle_tx, idle_rx) = broadcast::channel(1);

		let save_state_file_path = data_directory
			.as_ref()
			.join(SAMPLE_ACTOR_SAVE_STATE_FILE_NAME);

		let data = Arc::new(data);

		let pending_tasks = fs::read(&save_state_file_path)
			.await
			.map_err(|e| {
				if e.kind() == std::io::ErrorKind::NotFound {
					info!("No saved actor tasks found");
				} else {
					error!("Failed to read saved actor tasks: {e:#?}");
				}
			})
			.ok()
			.and_then(|data| {
				rmp_serde::from_slice::<Vec<SampleActorTaskSaveState>>(&data)
					.map_err(|e| {
						error!("Failed to deserialize saved actor tasks: {e:#?}");
					})
					.ok()
			})
			.unwrap_or_default();

		spawn(Self::run(save_state_file_path, task_handles_rx, idle_tx));

		for SampleActorTaskSaveState {
			id,
			duration,
			has_priority,
			paused_count,
		} in pending_tasks
		{
			task_handles_tx
				.send(if has_priority {
					task_dispatcher
						.dispatch(SampleActorTaskWithPriority::with_id(
							id,
							duration,
							Arc::clone(&data),
							paused_count,
						))
						.await
						.unwrap()
				} else {
					task_dispatcher
						.dispatch(SampleActorTask::with_id(
							id,
							duration,
							Arc::clone(&data),
							paused_count,
						))
						.await
						.unwrap()
				})
				.await
				.expect("Task handle receiver dropped");
		}

		(
			Self {
				data,
				task_dispatcher,
				task_handles_tx,
			},
			idle_rx,
		)
	}

	pub fn new_task(&self, duration: Duration) -> SampleActorTask {
		SampleActorTask::new(duration, Arc::clone(&self.data))
	}

	pub fn new_priority_task(&self, duration: Duration) -> SampleActorTaskWithPriority {
		SampleActorTaskWithPriority::new(duration, Arc::clone(&self.data))
	}

	async fn inner_process(&self, duration: Duration, has_priority: bool) {
		self.task_handles_tx
			.send(if has_priority {
				self.task_dispatcher
					.dispatch(self.new_priority_task(duration))
					.await
					.unwrap()
			} else {
				self.task_dispatcher
					.dispatch(self.new_task(duration))
					.await
					.unwrap()
			})
			.await
			.expect("Task handle receiver dropped");
	}

	pub async fn process(&self, duration: Duration) {
		self.inner_process(duration, false).await
	}

	pub async fn process_with_priority(&self, duration: Duration) {
		self.inner_process(duration, true).await
	}

	async fn run(
		save_state_file_path: PathBuf,
		task_handles_rx: chan::Receiver<TaskHandle<SampleError>>,
		idle_tx: broadcast::Sender<()>,
	) {
		let mut handles = FuturesUnordered::<TaskHandle<SampleError>>::new();

		enum RaceOutput {
			NewHandle(TaskHandle<SampleError>),
			CompletedHandle,
			Stop(Option<Box<dyn Task<SampleError>>>),
		}

		let mut pending = 0usize;

		loop {
			match (
				async {
					if let Ok(handle) = task_handles_rx.recv().await {
						RaceOutput::NewHandle(handle)
					} else {
						RaceOutput::Stop(None)
					}
				},
				async {
					if let Some(out) = handles.next().await {
						match out {
							Ok(TaskStatus::Done((_task_id, maybe_out))) => {
								if let TaskOutput::Out(out) = maybe_out {
									info!(
										"Task completed: {:?}",
										out.downcast::<TimedTaskOutput>()
											.expect("we know the task type")
									);
								}
							}
							Ok(TaskStatus::Canceled) => {
								trace!("Task was canceled")
							}
							Ok(TaskStatus::ForcedAbortion) => {
								warn!("Task was forcibly aborted");
							}
							Ok(TaskStatus::Shutdown(task)) => {
								// If a task was shutdown, it means the task system is shutting down
								// so all other tasks will also be shutdown

								return RaceOutput::Stop(Some(task));
							}
							Ok(TaskStatus::Error(e)) => {
								error!("Task failed: {e:#?}");
							}
							Err(e) => {
								error!("Task system failed: {e:#?}");
							}
						}

						RaceOutput::CompletedHandle
					} else {
						RaceOutput::Stop(None)
					}
				},
			)
				.race()
				.await
			{
				RaceOutput::NewHandle(handle) => {
					pending += 1;
					info!("Received new task handle, total pending tasks: {pending}");
					handles.push(handle);
				}
				RaceOutput::CompletedHandle => {
					pending -= 1;
					info!("Task completed, total pending tasks: {pending}");
					if pending == 0 {
						info!("All tasks completed, sending idle report...");
						idle_tx.send(()).expect("idle receiver dropped");
					}
				}
				RaceOutput::Stop(maybe_task) => {
					task_handles_rx.close();
					task_handles_rx
						.for_each(|handle| async { handles.push(handle) })
						.await;

					let tasks = stream::iter(
						maybe_task
							.into_iter()
							.map(SampleActorTaskSaveState::from_task),
					)
					.chain(handles.filter_map(|handle| async move {
						match handle {
							Ok(TaskStatus::Done((_task_id, maybe_out))) => {
								if let TaskOutput::Out(out) = maybe_out {
									info!(
										"Task completed: {:?}",
										out.downcast::<TimedTaskOutput>()
											.expect("we know the task type")
									);
								}

								None
							}
							Ok(TaskStatus::Canceled) => None,
							Ok(TaskStatus::ForcedAbortion) => {
								warn!("Task was forcibly aborted");
								None
							}
							Ok(TaskStatus::Shutdown(task)) => {
								Some(SampleActorTaskSaveState::from_task(task))
							}
							Ok(TaskStatus::Error(e)) => {
								error!("Task failed: {e:#?}");
								None
							}
							Err(e) => {
								error!("Task system failed: {e:#?}");
								None
							}
						}
					}))
					.collect::<Vec<_>>()
					.await;

					if let Err(e) = fs::write(
						&save_state_file_path,
						rmp_serde::to_vec_named(&tasks).expect("failed to serialize"),
					)
					.await
					{
						error!("Failed to save actor tasks: {e:#?}");
					}

					return;
				}
			}
		}
	}
}

impl Drop for SampleActor {
	fn drop(&mut self) {
		self.task_handles_tx.close();
	}
}

#[derive(Debug)]
pub struct SampleActorTask {
	timed_task: TimeTask,
	actor_data: Arc<String>, // Can hold any kind of actor data
}

impl SampleActorTask {
	pub fn new(duration: Duration, actor_data: Arc<String>) -> Self {
		Self {
			timed_task: TimeTask::new(duration, false),
			actor_data,
		}
	}

	fn with_id(id: TaskId, duration: Duration, actor_data: Arc<String>, paused_count: u32) -> Self {
		Self {
			timed_task: TimeTask::with_id(id, duration, false, paused_count),
			actor_data,
		}
	}
}

#[derive(Debug)]
pub struct SampleActorTaskWithPriority {
	timed_task: TimeTask,
	actor_data: Arc<String>, // Can hold any kind of actor data
}
impl SampleActorTaskWithPriority {
	fn new(duration: Duration, actor_data: Arc<String>) -> SampleActorTaskWithPriority {
		Self {
			timed_task: TimeTask::new(duration, true),
			actor_data,
		}
	}

	fn with_id(id: TaskId, duration: Duration, actor_data: Arc<String>, paused_count: u32) -> Self {
		Self {
			timed_task: TimeTask::with_id(id, duration, true, paused_count),
			actor_data,
		}
	}
}

#[async_trait]
impl Task<SampleError> for SampleActorTask {
	fn id(&self) -> TaskId {
		self.timed_task.id()
	}

	async fn run(&mut self, interrupter: &Interrupter) -> Result<ExecStatus, SampleError> {
		info!("Actor data: {:#?}", self.actor_data);
		let out = self.timed_task.run(interrupter).await?;
		if let ExecStatus::Done(TaskOutput::Out(out)) = &out {
			info!(
				"Task completed with {} pauses",
				out.downcast_ref::<TimedTaskOutput>()
					.expect("we know the task type")
					.pauses_count
			);
		}

		Ok(out)
	}

	fn with_priority(&self) -> bool {
		self.timed_task.with_priority()
	}
}

#[async_trait]
impl Task<SampleError> for SampleActorTaskWithPriority {
	fn id(&self) -> TaskId {
		self.timed_task.id()
	}

	async fn run(&mut self, interrupter: &Interrupter) -> Result<ExecStatus, SampleError> {
		info!("Actor data: {:#?}", self.actor_data);
		self.timed_task.run(interrupter).await
	}

	fn with_priority(&self) -> bool {
		self.timed_task.with_priority()
	}
}

#[derive(Debug, Serialize, Deserialize)]
struct SampleActorTaskSaveState {
	id: TaskId,
	duration: Duration,
	has_priority: bool,
	paused_count: u32,
}

impl SampleActorTaskSaveState {
	fn from_task(dyn_task: Box<dyn Task<SampleError>>) -> Self {
		match dyn_task.downcast::<SampleActorTask>() {
			Ok(concrete_task) => SampleActorTaskSaveState {
				id: concrete_task.timed_task.id(),
				duration: concrete_task.timed_task.duration,
				has_priority: false,
				paused_count: concrete_task.timed_task.paused_count,
			},
			Err(dyn_task) => {
				let concrete_task = dyn_task
					.downcast::<SampleActorTaskWithPriority>()
					.expect("we know the task type");

				SampleActorTaskSaveState {
					id: concrete_task.timed_task.id(),
					duration: concrete_task.timed_task.duration,
					has_priority: true,
					paused_count: concrete_task.timed_task.paused_count,
				}
			}
		}
	}
}
