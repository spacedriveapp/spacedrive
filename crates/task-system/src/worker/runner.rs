use std::{
	collections::{HashMap, VecDeque},
	future::pending,
	pin::pin,
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
};

use async_channel as chan;
use futures::StreamExt;
use futures_concurrency::future::Race;
use tokio::{
	spawn,
	sync::oneshot,
	task::{JoinError, JoinHandle},
	time::{timeout, Instant},
};
use tracing::{debug, error, trace, warn};

use super::{
	super::{
		error::Error,
		system::SystemComm,
		task::{DynTask, ExecStatus, InternalTaskExecStatus, TaskId, TaskStatus, TaskWorkState},
	},
	RunnerMessage, TaskRunnerOutput, WorkStealer, WorkerId, ONE_SECOND,
};

pub(super) enum TaskAddStatus {
	Running,
	Enqueued,
}

struct AbortAndSuspendSignalers {
	abort_tx: oneshot::Sender<oneshot::Sender<Result<(), Error>>>,
	suspend_tx: oneshot::Sender<()>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PendingTaskKind {
	Normal,
	Priority,
	Suspended,
}

impl PendingTaskKind {
	fn with_priority(has_priority: bool) -> Self {
		if has_priority {
			Self::Priority
		} else {
			Self::Normal
		}
	}
}

struct RunningTask {
	task_id: TaskId,
	task_kind: PendingTaskKind,
	handle: JoinHandle<()>,
}

fn dispatch_steal_request(
	worker_id: WorkerId,
	work_stealer: WorkStealer,
	runner_tx: chan::Sender<RunnerMessage>,
) -> JoinHandle<()> {
	spawn(async move {
		runner_tx
			.send(RunnerMessage::StealedTask(
				work_stealer.steal(worker_id).await,
			))
			.await
			.expect("runner channel closed before send stealed task");
	})
}

enum WaitingSuspendedTask {
	Task(TaskId),
	None,
}

impl WaitingSuspendedTask {
	fn is_waiting(&self) -> bool {
		matches!(self, Self::Task(_))
	}
}

pub(super) struct Runner {
	worker_id: WorkerId,
	system_comm: SystemComm,
	work_stealer: WorkStealer,
	task_kinds: HashMap<TaskId, PendingTaskKind>,
	tasks: VecDeque<TaskWorkState>,
	paused_tasks: HashMap<TaskId, TaskWorkState>,
	suspended_task: Option<TaskWorkState>,
	priority_tasks: VecDeque<TaskWorkState>,
	last_requested_help: Instant,
	is_idle: bool,
	waiting_suspension: WaitingSuspendedTask,
	abort_and_suspend_map: HashMap<TaskId, AbortAndSuspendSignalers>,
	runner_tx: chan::Sender<RunnerMessage>,
	current_task_handle: Option<RunningTask>,
	suspend_on_shutdown_rx: chan::Receiver<RunnerMessage>,
	pub(super) current_steal_task_handle: Option<JoinHandle<()>>,
}

impl Runner {
	pub(super) fn new(
		worker_id: WorkerId,
		work_stealer: WorkStealer,
		system_comm: SystemComm,
	) -> (Self, chan::Receiver<RunnerMessage>) {
		let (runner_tx, runner_rx) = chan::bounded(8);

		(
			Self {
				worker_id,
				system_comm,
				work_stealer,
				task_kinds: HashMap::with_capacity(64),
				tasks: VecDeque::with_capacity(64),
				paused_tasks: HashMap::new(),
				suspended_task: None,
				priority_tasks: VecDeque::with_capacity(32),
				last_requested_help: Instant::now(),
				is_idle: true,
				waiting_suspension: WaitingSuspendedTask::None,
				abort_and_suspend_map: HashMap::with_capacity(8),
				runner_tx,
				current_task_handle: None,
				suspend_on_shutdown_rx: runner_rx.clone(),
				current_steal_task_handle: None,
			},
			runner_rx,
		)
	}

	pub(super) fn total_tasks(&self) -> usize {
		let priority_tasks_count = self.priority_tasks.len();
		let current_task_count = if self.current_task_handle.is_some() {
			1
		} else {
			0
		};
		let suspended_task_count = if self.suspended_task.is_some() { 1 } else { 0 };
		let tasks_count = self.tasks.len();

		trace!(
			"Task count: \
			<worker_id='{}', \
			priority_tasks_count={priority_tasks_count}, \
			current_task_count={current_task_count}, \
			suspended_task_count={suspended_task_count}, \
			tasks_count={tasks_count}>",
			self.worker_id
		);

		priority_tasks_count + current_task_count + suspended_task_count + tasks_count
	}

	pub(super) fn spawn_task_runner(
		&mut self,
		task_id: TaskId,
		task_work_state: TaskWorkState,
	) -> JoinHandle<()> {
		let (abort_tx, abort_rx) = oneshot::channel();
		let (suspend_tx, suspend_rx) = oneshot::channel();

		self.abort_and_suspend_map.insert(
			task_id,
			AbortAndSuspendSignalers {
				abort_tx,
				suspend_tx,
			},
		);

		let handle = spawn(run_single_task(
			self.worker_id,
			task_work_state,
			self.runner_tx.clone(),
			suspend_rx,
			abort_rx,
		));

		trace!(
			"Task runner spawned: <worker_id='{}', task_id='{task_id}'>",
			self.worker_id
		);

		handle
	}

	pub(super) async fn new_task(&mut self, task_work_state: TaskWorkState) {
		let task_id = task_work_state.task.id();
		let new_kind = PendingTaskKind::with_priority(task_work_state.task.with_priority());

		trace!(
			"Received new task: <worker_id='{}', task_id='{task_id}', kind='{new_kind:#?}'>",
			self.worker_id
		);

		self.task_kinds.insert(task_id, new_kind);

		match self
			.inner_add_task(task_id, new_kind, task_work_state)
			.await
		{
			TaskAddStatus::Running => trace!(
				"Task running: <worker_id='{}', task_id='{task_id}'>",
				self.worker_id
			),
			TaskAddStatus::Enqueued => trace!(
				"Task enqueued: <worker_id='{}', task_id='{task_id}'>",
				self.worker_id
			),
		}
	}

	pub(super) async fn resume_task(&mut self, task_id: TaskId) -> Result<(), Error> {
		trace!(
			"Resume task request: <worker_id='{}', task_id='{task_id}'>",
			self.worker_id
		);
		if let Some(task_work_state) = self.paused_tasks.remove(&task_id) {
			task_work_state.worktable.set_resumed();

			match self
				.inner_add_task(
					task_id,
					*self
						.task_kinds
						.get(&task_id)
						.expect("we added the task kind before pausing it"),
					task_work_state,
				)
				.await
			{
				TaskAddStatus::Running => trace!(
					"Resumed task is running: <worker_id='{}', task_id='{task_id}'>",
					self.worker_id
				),
				TaskAddStatus::Enqueued => trace!(
					"Resumed task was enqueued: <worker_id='{}', task_id='{task_id}'>",
					self.worker_id
				),
			}

			Ok(())
		} else {
			trace!(
				"Task not found: <worker_id='{}', task_id='{task_id}'>",
				self.worker_id
			);
			Err(Error::TaskNotFound(task_id))
		}
	}

	// TODO: Preciso de algum jeito de notificar q estou esperando uma task ser suspensa e q se chegar task nova, tem q enfileirar

	#[inline(always)]
	pub(super) async fn inner_add_task(
		&mut self,
		task_id: TaskId,
		task_kind: PendingTaskKind,
		task_work_state: TaskWorkState,
	) -> TaskAddStatus {
		if self.is_idle {
			trace!(
				"Idle worker will process the new task: <worker_id='{}', task_id='{task_id}'>",
				self.worker_id
			);
			let handle = self.spawn_task_runner(task_id, task_work_state);

			self.current_task_handle = Some(RunningTask {
				task_id,
				task_kind,
				handle,
			});

			// Doesn't need to report working back to system as it already registered
			// that we're not idle anymore when it dispatched the task to this worker
			self.is_idle = false;

			TaskAddStatus::Running
		} else {
			let RunningTask {
				task_id: old_task_id,
				task_kind: old_kind,
				..
			} = self
				.current_task_handle
				.as_ref()
				.expect("Worker isn't idle, but no task is running");

			trace!(
				"Worker is busy: \
				<worker_id='{}', task_id='{task_id}', current_task_kind='{old_kind:#?}'>",
				self.worker_id,
			);

			let add_status = match (task_kind, old_kind) {
				(PendingTaskKind::Priority, PendingTaskKind::Priority) => {
					trace!(
						"Old and new tasks have priority, will put new task on priority queue: \
						<worker_id='{}', task_id='{task_id}'>",
						self.worker_id
					);
					self.priority_tasks.push_front(task_work_state);

					TaskAddStatus::Enqueued
				}
				(PendingTaskKind::Priority, PendingTaskKind::Normal) => {
					if !self.waiting_suspension.is_waiting() {
						trace!(
							"Old task will be suspended: \
						<worker_id='{}', new_task_id='{task_id}', old_task_id='{old_task_id}'>",
							self.worker_id
						);

						// We put the query at the top of the priority queue, so it will be
						// dispatched by the StreamMessage::TaskOutput handler below
						self.priority_tasks.push_front(task_work_state);

						if self
							.abort_and_suspend_map
							.remove(old_task_id)
							.expect("we always store the abort and suspend signalers")
							.suspend_tx
							.send(())
							.is_err()
						{
							warn!(
							"Task <id='{old_task_id}'> suspend channel closed before receiving suspend signal. \
							This probably happened because the task finished before we could suspend it."
						);
						}

						self.waiting_suspension = WaitingSuspendedTask::Task(*old_task_id);
					} else {
						trace!(
							"Worker is already waiting for a task to be suspended, will enqueue new task: \
							<worker_id='{}', task_id='{task_id}'>",
							self.worker_id
						);

						self.priority_tasks.push_front(task_work_state);
					}

					TaskAddStatus::Running
				}
				(_, _) => {
					trace!(
						"New task doesn't have priority and will be enqueued: \
						<worker_id='{}', task_id='{task_id}'>",
						self.worker_id,
					);

					self.tasks.push_back(task_work_state);

					TaskAddStatus::Enqueued
				}
			};

			let task_count = self.total_tasks();

			trace!(
				"Worker with {task_count} pending tasks: <worker_id='{}'>",
				self.worker_id
			);

			if task_count > self.work_stealer.workers_count()
				&& self.last_requested_help.elapsed() > ONE_SECOND
			{
				trace!(
					"Worker requesting help from the system: \
					<worker_id='{}', task_count='{task_count}'>",
					self.worker_id
				);

				self.system_comm
					.request_help(self.worker_id, task_count)
					.await;

				self.last_requested_help = Instant::now();
			}

			add_status
		}
	}

	pub(super) async fn force_task_abortion(&mut self, task_id: uuid::Uuid) -> Result<(), Error> {
		if let Some(AbortAndSuspendSignalers { abort_tx, .. }) =
			self.abort_and_suspend_map.remove(&task_id)
		{
			let (tx, rx) = oneshot::channel();

			if abort_tx.send(tx).is_err() {
				debug!(
					"Failed to send force abortion request, the task probably finished before we could abort it: \
					<worker_id='{}', task_id='{task_id}'>",
					self.worker_id
				);

				Ok(())
			} else {
				match timeout(ONE_SECOND, rx).await {
					Ok(Ok(res)) => res,
					// If the sender was dropped, then the task finished before we could
					// abort it which is fine
					Ok(Err(_)) => Ok(()),
					Err(_) => Err(Error::TaskForcedAbortTimeout(task_id)),
				}
			}
		} else {
			Err(Error::TaskNotFound(task_id))
		}
	}

	pub(super) async fn shutdown(self, tx: oneshot::Sender<()>) {
		let Runner {
			worker_id,
			tasks,
			paused_tasks,
			priority_tasks,
			is_idle,
			abort_and_suspend_map,
			runner_tx,
			mut current_task_handle,
			suspend_on_shutdown_rx,
			..
		} = self;

		trace!("Worker beginning shutdown process: <worker_id='{worker_id}'>");

		let mut suspend_on_shutdown_rx = pin!(suspend_on_shutdown_rx);

		if !is_idle {
			trace!("Worker is busy, will shutdown tasks: <worker_id='{worker_id}'>");

			if let Some(RunningTask {
				task_id, handle, ..
			}) = current_task_handle.take()
			{
				abort_and_suspend_map.into_iter().for_each(
					|(task_id, AbortAndSuspendSignalers { suspend_tx, .. })| {
						if suspend_tx.send(()).is_err() {
							warn!(
								"Shutdown request channel closed before sending abort signal: \
								<worker_id='{worker_id}', task_id='{task_id}'>"
							);
						} else {
							trace!(
								"Sent suspend signal for task on shutdown: \
								<worker_id='{worker_id}', task_id='{task_id}'>"
							);
						}
					},
				);

				if let Err(e) = handle.await {
					error!("Task <worker_id='{worker_id}', task_id='{task_id}'> failed to join: {e:#?}");
				}

				runner_tx.close();

				while let Some(runner_msg) = suspend_on_shutdown_rx.next().await {
					match runner_msg {
						RunnerMessage::TaskOutput(task_id, res) => match res {
							Ok(TaskRunnerOutput {
								task_work_state: TaskWorkState { task, done_tx, .. },
								status,
							}) => match status {
								InternalTaskExecStatus::Done => {
									if done_tx.send(Ok(TaskStatus::Done)).is_err() {
										warn!(
										"Task done channel closed before sending done response for task: \
										<worker_id='{worker_id}', task_id='{task_id}'>"
									);
									} else {
										trace!(
											"Emitted task done signal on shutdown: \
										<worker_id='{worker_id}', task_id='{task_id}'>"
										);
									}
								}

								InternalTaskExecStatus::Canceled => {
									if done_tx.send(Ok(TaskStatus::Canceled)).is_err() {
										warn!(
										"Task done channel closed before sending canceled response for task: \
										<worker_id='{worker_id}', task_id='{task_id}'>"
									);
									} else {
										trace!(
											"Emitted task canceled signal on shutdown: \
										<worker_id='{worker_id}', task_id='{task_id}'>"
										);
									}
								}

								InternalTaskExecStatus::Suspend
								| InternalTaskExecStatus::Paused => {
									if done_tx.send(Ok(TaskStatus::Shutdown(task))).is_err() {
										warn!(
										"Task done channel closed before sending shutdown response for task: \
										<worker_id='{worker_id}', task_id='{task_id}'>"
									);
									} else {
										trace!(
										"Sucessfully suspended and sent back DynTask on worker shutdown: \
										<worker_id='{worker_id}', task_id='{task_id}'>"
									);
									}
								}
							},
							Err(e) => {
								error!(
								"Task <worker_id='{worker_id}', task_id='{task_id}'> failed to suspend on shutdown: \
								{e:#?}"
							);
							}
						},
						RunnerMessage::StealedTask(Some(TaskWorkState {
							task, done_tx, ..
						})) => {
							if done_tx.send(Ok(TaskStatus::Shutdown(task))).is_err() {
								warn!(
									"Task done channel closed before sending shutdown response for task: \
									<worker_id='{worker_id}', task_id='{task_id}'>"
								);
							} else {
								trace!(
									"Sucessfully suspended and sent back DynTask on worker shutdown: \
									<worker_id='{worker_id}', task_id='{task_id}'>"
								);
							}
						}
						RunnerMessage::StealedTask(None) => {}
					}
				}
			}

			priority_tasks
				.into_iter()
				.chain(paused_tasks.into_values())
				.chain(tasks.into_iter())
				.for_each(|TaskWorkState { task, done_tx, .. }| {
					let task_id = task.id();
					if done_tx.send(Ok(TaskStatus::Shutdown(task))).is_err() {
						warn!(
							"Task done channel closed before sending shutdown response for task: \
							<worker_id='{worker_id}', task_id='{task_id}'>"
						);
					} else {
						trace!(
							"Sucessfully sent back DynTask on worker shutdown: \
							<worker_id='{worker_id}', task_id='{task_id}'>"
						);
					}
				})
		} else {
			trace!("Worker is idle, no tasks to shutdown: <worker_id='{worker_id}'>");
		}

		trace!("Worker shutdown process completed: <worker_id='{worker_id}'>");

		if tx.send(()).is_err() {
			warn!("Shutdown request channel closed before sending ack");
		}
	}

	pub(super) fn get_next_task(&mut self) -> Option<(PendingTaskKind, TaskWorkState)> {
		if let Some(task) = self.priority_tasks.pop_front() {
			return Some((PendingTaskKind::Priority, task));
		}

		if let Some(task) = self.suspended_task.take() {
			task.interrupter.reset();
			return Some((PendingTaskKind::Suspended, task));
		}

		self.tasks
			.pop_front()
			.map(|task| (PendingTaskKind::Normal, task))
	}

	pub(super) fn steal_request(&mut self, tx: oneshot::Sender<Option<TaskWorkState>>) {
		trace!("Steal request: <worker_id='{}'>", self.worker_id);
		if let Some((kind, task)) = self.get_next_task() {
			let task_id = task.task.id();
			self.task_kinds.remove(&task_id);

			trace!(
				"Stealing task: <worker_id='{}', task_id='{task_id}', kind='{kind:#?}'>",
				self.worker_id
			);

			if let Err(Some(task)) = tx.send(Some(task)) {
				warn!(
					"Steal request channel closed before sending task: <worker_id='{}'>",
					self.worker_id
				);
				match kind {
					PendingTaskKind::Normal => self.tasks.push_front(task),
					PendingTaskKind::Priority => self.priority_tasks.push_front(task),
					PendingTaskKind::Suspended => self.suspended_task = Some(task),
				}

				self.task_kinds.insert(task_id, kind);
			}
		} else {
			trace!("No task to steal: <worker_id='{}'>", self.worker_id);
			if tx.send(None).is_err() {
				warn!(
					"Steal request channel closed before sending no task response: \
					<worker_id='{}'>",
					self.worker_id
				);
			}
		}
	}

	pub(super) async fn wake_up(&mut self) {
		if self.is_idle {
			trace!(
				"Worker is idle, waking up: <worker_id='{}'>",
				self.worker_id
			);

			if self.current_steal_task_handle.is_none() {
				self.current_steal_task_handle = Some(dispatch_steal_request(
					self.worker_id,
					self.work_stealer.clone(),
					self.runner_tx.clone(),
				));
			} else {
				trace!(
					"Steal task already running, ignoring wake up request: <worker_id='{}'>",
					self.worker_id
				);
			}
		} else {
			trace!(
				"Worker already working, ignoring wake up request: <worker_id='{}'>",
				self.worker_id
			);
		}
	}

	#[inline(always)]
	pub(super) async fn dispatch_next_task(&mut self, finished_task_id: TaskId) {
		trace!(
			"Task finished and will try to process a new task: \
			<worker_id='{}', finished_task_id='{finished_task_id}'>",
			self.worker_id
		);

		self.abort_and_suspend_map.remove(&finished_task_id);

		let RunningTask {
			task_id: old_task_id,

			handle,
			..
		} = self
			.current_task_handle
			.take()
			.expect("Task handle missing, but task output received");

		assert_eq!(finished_task_id, old_task_id, "Task output id mismatch");

		trace!(
			"Waiting task handle: <worker_id='{}', task_id='{old_task_id}'>",
			self.worker_id
		);
		if let Err(e) = handle.await {
			error!("Task <id='{old_task_id}'> failed to join: {e:#?}");
		}
		trace!(
			"Waited task handle: <worker_id='{}', task_id='{old_task_id}'>",
			self.worker_id
		);

		if let Some((task_kind, task_work_state)) = self.get_next_task() {
			let task_id = task_work_state.task.id();

			trace!(
				"Dispatching next task: <worker_id='{}', task_id='{task_id}', kind='{task_kind:#?}'>",
				self.worker_id
			);

			let handle = self.spawn_task_runner(task_id, task_work_state);

			self.current_task_handle = Some(RunningTask {
				task_id,
				task_kind,
				handle,
			});
		} else {
			trace!(
				"No task to dispatch, worker is now idle and will dispatch a steal request: <worker_id='{}'>",
				self.worker_id
			);

			self.is_idle = true;
			self.system_comm.idle_report(self.worker_id).await;

			if self.current_steal_task_handle.is_none() {
				self.current_steal_task_handle = Some(dispatch_steal_request(
					self.worker_id,
					self.work_stealer.clone(),
					self.runner_tx.clone(),
				));
			} else {
				trace!(
					"Steal task already running: <worker_id='{}'>",
					self.worker_id
				);
			}
		}
	}

	pub(super) async fn process_task_output(
		&mut self,
		task_id: TaskId,
		TaskRunnerOutput {
			task_work_state:
				TaskWorkState {
					task,
					worktable,
					done_tx,
					interrupter,
				},
			status,
		}: TaskRunnerOutput,
	) {
		match status {
			InternalTaskExecStatus::Done => {
				worktable.set_completed();
				if done_tx.send(Ok(TaskStatus::Done)).is_err() {
					warn!("Task done channel closed before sending done response for task <id='{task_id}'>");
				} else {
					trace!(
						"Task done signal emitted: <worker_id='{}', task_id='{task_id}'>",
						self.worker_id
					);
				}
			}
			InternalTaskExecStatus::Paused => {
				self.paused_tasks.insert(
					task_id,
					TaskWorkState {
						task,
						worktable,
						done_tx,
						interrupter,
					},
				);
				trace!(
					"Task paused: <worker_id='{}', task_id='{task_id}'>",
					self.worker_id
				);
			}
			InternalTaskExecStatus::Canceled => {
				if done_tx.send(Ok(TaskStatus::Canceled)).is_err() {
					warn!("Task done channel closed before sending cancelled response for task <id='{task_id}'>");
				} else {
					trace!(
						"Task canceled signal emitted: <worker_id='{}', task_id='{task_id}'>",
						self.worker_id
					);
				}
			}
			InternalTaskExecStatus::Suspend => {
				self.suspended_task = Some(TaskWorkState {
					task,
					worktable,
					done_tx,
					interrupter,
				});
				trace!(
					"Task suspended: <worker_id='{}', task_id='{task_id}'>",
					self.worker_id
				);

				self.clean_suspended_task(task_id);
			}
		}

		trace!(
			"Processing task output completed and will try to dispatch a new task: \
			<worker_id='{}', task_id='{task_id}'>",
			self.worker_id
		);

		self.dispatch_next_task(task_id).await;
	}

	pub(super) async fn idle_check(&mut self) {
		if self.is_idle {
			trace!(
				"Worker is idle for some time and will try to steal a task: <worker_id='{}'>",
				self.worker_id
			);

			if self.current_steal_task_handle.is_none() {
				self.current_steal_task_handle = Some(dispatch_steal_request(
					self.worker_id,
					self.work_stealer.clone(),
					self.runner_tx.clone(),
				));
			} else {
				trace!(
					"Steal task already running, ignoring on this idle check: <worker_id='{}'>",
					self.worker_id
				);
			}
		}
	}

	pub(super) fn abort_steal_task(&mut self) {
		if let Some(steal_task_handle) = self.current_steal_task_handle.take() {
			trace!(
				"Worker received a new task while a steal task was running, will abort it: \
				<worker_id='{}'>",
				self.worker_id
			);
			steal_task_handle.abort();
		}
	}

	pub(super) async fn process_stealed_task(&mut self, maybe_new_task: Option<TaskWorkState>) {
		if let Some(steal_task_handle) = self.current_steal_task_handle.take() {
			if let Err(e) = steal_task_handle.await {
				error!("Steal task failed to join: {e:#?}");
			}
		}

		if let Some(task_work_state) = maybe_new_task {
			self.system_comm.working_report(self.worker_id).await;
			self.new_task(task_work_state).await;
		}
	}

	pub(crate) fn clean_suspended_task(&mut self, task_id: uuid::Uuid) {
		match self.waiting_suspension {
			WaitingSuspendedTask::Task(waiting_task_id) if waiting_task_id == task_id => {
				trace!(
					"Task was suspended and will be cleaned: <worker_id='{}', task_id='{task_id}'>",
					self.worker_id
				);
				self.waiting_suspension = WaitingSuspendedTask::None;
			}
			WaitingSuspendedTask::Task(_) => {
				trace!(
					"Task wasn't suspended, ignoring: <worker_id='{}', task_id='{task_id}'>",
					self.worker_id
				);
			}
			WaitingSuspendedTask::None => {}
		}
	}
}

async fn run_single_task(
	worker_id: WorkerId,
	TaskWorkState {
		mut task,
		worktable,
		interrupter,
		done_tx,
	}: TaskWorkState,
	runner_tx: chan::Sender<RunnerMessage>,
	suspend_rx: oneshot::Receiver<()>,
	abort_rx: oneshot::Receiver<oneshot::Sender<Result<(), Error>>>,
) {
	let task_id = task.id();

	worktable.set_started();

	trace!("Running task: <worker_id='{worker_id}', task_id='{task_id}'>");

	let handle = spawn({
		let interrupter = Arc::clone(&interrupter);
		async move {
			let res = task.run(&interrupter).await;

			trace!("Ran task: <worker_id='{worker_id}', task_id='{task_id}'>: {res:?}");

			(task, res)
		}
	});

	let abort_handle = handle.abort_handle();

	let has_suspended = Arc::new(AtomicBool::new(false));

	let suspender_handle = spawn({
		let has_suspended = Arc::clone(&has_suspended);
		let worktable = Arc::clone(&worktable);
		async move {
			if suspend_rx.await.is_ok() {
				let (tx, rx) = oneshot::channel();

				trace!("Suspend signal received: <worker_id='{worker_id}', task_id='{task_id}'>");

				// The interrupter only knows about Pause and Cancel commands, we use pause as
				// the suspend task feature should be invisible to the user
				worktable.pause(tx).await;

				match rx.await {
					Ok(Ok(())) => {
						trace!("Suspending: <worker_id='{worker_id}', task_id='{task_id}'>");
						has_suspended.store(true, Ordering::Relaxed);
					}
					Ok(Err(e)) => {
						error!(
							"Task <worker_id='{worker_id}', task_id='{task_id}'> failed to suspend: {e:#?}",
						);
					}
					Err(_) => {
						// The task probably finished before we could suspend it so the channel was dropped
						trace!("Suspend channel closed: <worker_id='{worker_id}', task_id='{task_id}'>");
					}
				}
			}
		}
	});

	enum RaceOutput {
		Completed(Result<(DynTask, Result<ExecStatus, Error>), JoinError>),
		Abort(oneshot::Sender<Result<(), Error>>),
	}

	match (async { RaceOutput::Completed(handle.await) }, async move {
		if let Ok(tx) = abort_rx.await {
			trace!("Aborting task: <worker_id='{worker_id}', task_id='{task_id}'>");
			RaceOutput::Abort(tx)
		} else {
			// If the abort channel is closed, we should just ignore it and keep waiting for the task to finish
			// as we're being suspended by the worker
			trace!(
				"Abort channel closed, will wait for task to finish: <worker_id='{worker_id}', task_id='{task_id}'>"
			);
			pending().await
		}
	})
		.race()
		.await
	{
		RaceOutput::Completed(Ok((task, res))) => {
			trace!("Task completed ok: <worker_id='{worker_id}', task_id='{task_id}'>");
			runner_tx
				.send(RunnerMessage::TaskOutput(
					task_id,
					res.map(|status| {
						let mut status = status.into();
						if status == InternalTaskExecStatus::Paused
							&& has_suspended.load(Ordering::Relaxed)
						{
							status = InternalTaskExecStatus::Suspend;
						}

						TaskRunnerOutput {
							task_work_state: TaskWorkState {
								task,
								worktable,
								interrupter,
								done_tx,
							},
							status,
						}
					}),
				))
				.await
				.expect("Task runner channel closed while sending task output");
		}

		RaceOutput::Completed(Err(join_error)) => {
			error!("Task <id='{task_id}'> failed to join: {join_error:#?}",);
			if done_tx.send(Err(Error::TaskJoin(task_id))).is_err() {
				error!("Task done channel closed while sending join error response");
			}

			if runner_tx
				.send(RunnerMessage::TaskOutput(
					task_id,
					Err(Error::TaskJoin(task_id)),
				))
				.await
				.is_err()
			{
				error!("Task runner channel closed while sending join error response");
			}
		}

		RaceOutput::Abort(tx) => {
			abort_handle.abort();

			trace!("Task aborted: <worker_id='{worker_id}', task_id='{task_id}'>");

			if done_tx.send(Err(Error::TaskAborted(task_id))).is_err() {
				error!("Task done channel closed while sending abort error response");
			}

			if runner_tx
				.send(RunnerMessage::TaskOutput(
					task_id,
					Err(Error::TaskAborted(task_id)),
				))
				.await
				.is_err()
			{
				error!("Task runner channel closed while sending abort error response");
			}

			if tx.send(Ok(())).is_err() {
				error!("Task abort channel closed while sending abort error response");
			}
		}
	}

	if !suspender_handle.is_finished() {
		trace!(
			"Aborting suspender handler as it isn't needed anymore: <worker_id='{worker_id}', task_id='{task_id}'>"
		);
		// if we received a suspend signal this abort will do nothing, as the task finished already
		suspender_handle.abort();
	}

	trace!("Run single task finished: <worker_id='{worker_id}', task_id='{task_id}'>");
}
