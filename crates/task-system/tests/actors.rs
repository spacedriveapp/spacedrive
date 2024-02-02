use sd_task_system::{
	DynTask, ExecStatus, Interrupter, InterruptionKind, Task, TaskDispatcher, TaskHandle, TaskId,
	TaskStatus, TaskSystemError,
};
use serde::{Deserialize, Serialize};

use std::{
	path::{Path, PathBuf},
	sync::Arc,
	time::Duration,
};

use async_channel as chan;
use async_trait::async_trait;
use futures::stream::{self, FuturesUnordered, StreamExt};
use futures_concurrency::future::Race;
use tokio::{
	fs, spawn,
	time::{sleep, Instant},
};
use tracing::{error, info, warn};

const SAMPLE_ACTOR_SAVE_STATE_FILE_NAME: &str = "sample_actor_save_state.bin";

pub struct SampleActor {
	data: Arc<String>, // Can hold any kind of actor data, like an AI model
	task_dispatcher: TaskDispatcher,
	task_handles_tx: chan::Sender<TaskHandle>,
}

impl SampleActor {
	pub async fn new(
		data_directory: impl AsRef<Path>,
		data: String,
		task_dispatcher: TaskDispatcher,
	) -> Self {
		let (task_handles_tx, task_handles_rx) = chan::bounded(8);

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

		spawn(Self::run(save_state_file_path, task_handles_rx));

		for SampleActorTaskSaveState {
			id,
			duration,
			has_priority,
		} in pending_tasks
		{
			task_handles_tx
				.send(if has_priority {
					task_dispatcher
						.dispatch(SampleActorTaskWithPriority {
							id,
							duration,
							actor_data: Arc::clone(&data),
						})
						.await
				} else {
					task_dispatcher
						.dispatch(SampleActorTask {
							id,
							duration,
							actor_data: Arc::clone(&data),
						})
						.await
				})
				.await
				.expect("Task handle receiver dropped");
		}

		Self {
			data,
			task_dispatcher,
			task_handles_tx,
		}
	}

	pub fn new_task(&self, duration: Duration) -> SampleActorTask {
		SampleActorTask {
			id: TaskId::new_v4(),
			duration,
			actor_data: Arc::clone(&self.data),
		}
	}

	pub fn new_priority_task(&self, duration: Duration) -> SampleActorTaskWithPriority {
		SampleActorTaskWithPriority {
			id: TaskId::new_v4(),
			duration,
			actor_data: Arc::clone(&self.data),
		}
	}

	async fn inner_process(&self, duration: Duration, has_priority: bool) {
		self.task_handles_tx
			.send(if has_priority {
				self.task_dispatcher
					.dispatch(self.new_priority_task(duration))
					.await
			} else {
				self.task_dispatcher.dispatch(self.new_task(duration)).await
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

	async fn run(save_state_file_path: PathBuf, task_handles_rx: chan::Receiver<TaskHandle>) {
		let mut handles = FuturesUnordered::<TaskHandle>::new();

		enum RaceOutput {
			NewHandle(TaskHandle),
			CompletedHandle,
			Stop(Option<DynTask>),
		}

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
							Ok(TaskStatus::Done) | Ok(TaskStatus::Canceled) => {}
							Ok(TaskStatus::ForcedAbortion) => {
								warn!("Task was forcibly aborted");
							}
							Ok(TaskStatus::Shutdown(task)) => {
								// If a task was shutdown, it means the task system is shutting down
								// so all other tasks will also be shutdown

								return RaceOutput::Stop(Some(task));
							}
							Err(e) => {
								error!("Task failed: {e:#?}");
							}
						}
					}

					RaceOutput::CompletedHandle
				},
			)
				.race()
				.await
			{
				RaceOutput::NewHandle(handle) => {
					handles.push(handle);
				}
				RaceOutput::CompletedHandle => {}
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
							Ok(TaskStatus::Done) | Ok(TaskStatus::Canceled) => None,
							Ok(TaskStatus::ForcedAbortion) => {
								warn!("Task was forcibly aborted");
								None
							}
							Ok(TaskStatus::Shutdown(task)) => {
								Some(SampleActorTaskSaveState::from_task(task))
							}
							Err(e) => {
								error!("Task failed: {e:#?}");
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
	id: TaskId,
	duration: Duration,
	actor_data: Arc<String>, // Can hold any kind of actor data
}

#[derive(Debug)]
pub struct SampleActorTaskWithPriority {
	id: TaskId,
	duration: Duration,
	actor_data: Arc<String>, // Can hold any kind of actor data
}

#[async_trait]
impl Task for SampleActorTask {
	fn id(&self) -> TaskId {
		self.id
	}

	async fn run(&mut self, interrupter: &Interrupter) -> Result<ExecStatus, TaskSystemError> {
		run_actor_task(&mut self.duration, &self.actor_data, interrupter).await
	}
}

#[async_trait]
impl Task for SampleActorTaskWithPriority {
	fn id(&self) -> TaskId {
		self.id
	}

	async fn run(&mut self, interrupter: &Interrupter) -> Result<ExecStatus, TaskSystemError> {
		run_actor_task(&mut self.duration, &self.actor_data, interrupter).await
	}

	fn with_priority(&self) -> bool {
		true
	}
}

async fn run_actor_task(
	task_duration: &mut Duration,
	actor_data: &str,
	interrupter: &Interrupter,
) -> Result<ExecStatus, TaskSystemError> {
	let start = Instant::now();

	info!("Running actor task for {task_duration:#?}; Data: {actor_data}");

	enum RaceOutput {
		Paused(Duration),
		Canceled,
		Completed,
	}

	match (
		async {
			sleep(*task_duration).await;
			RaceOutput::Completed
		},
		async {
			match interrupter.await {
				InterruptionKind::Pause => RaceOutput::Paused(*task_duration - start.elapsed()),
				InterruptionKind::Cancel => RaceOutput::Canceled,
			}
		},
	)
		.race()
		.await
	{
		RaceOutput::Paused(remaining_duration) => {
			*task_duration = remaining_duration;
			Ok(ExecStatus::Paused)
		}
		RaceOutput::Canceled => Ok(ExecStatus::Canceled),
		RaceOutput::Completed => Ok(ExecStatus::Done),
	}
}

#[derive(Debug, Serialize, Deserialize)]
struct SampleActorTaskSaveState {
	id: TaskId,
	duration: Duration,
	has_priority: bool,
}

impl SampleActorTaskSaveState {
	fn from_task(dyn_task: Box<dyn Task>) -> Self {
		match dyn_task.downcast::<SampleActorTask>() {
			Ok(concrete_task) => SampleActorTaskSaveState {
				id: concrete_task.id,
				duration: concrete_task.duration,
				has_priority: false,
			},
			Err(dyn_task) => {
				let concrete_task = dyn_task
					.downcast::<SampleActorTaskWithPriority>()
					.expect("we know the task type");

				SampleActorTaskSaveState {
					id: concrete_task.id,
					duration: concrete_task.duration,
					has_priority: true,
				}
			}
		}
	}
}
