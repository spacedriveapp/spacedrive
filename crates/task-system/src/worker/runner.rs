use std::{
	any::Any,
	collections::{HashMap, VecDeque},
	future::pending,
	panic::AssertUnwindSafe,
	pin::pin,
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
	time::Duration,
};

use async_channel as chan;
use futures::{FutureExt, StreamExt};
use futures_concurrency::{future::Race, stream::Merge};
use tokio::{
	spawn,
	sync::oneshot,
	task::{JoinError, JoinHandle},
	time::{sleep, timeout, Instant},
};
use tracing::{debug, error, instrument, trace, warn, Instrument};

use super::{
	super::{
		error::{RunError, SystemError},
		message::{StoleTaskMessage, TaskOutputMessage},
		system::SystemComm,
		task::{
			ExecStatus, InternalTaskExecStatus, Interrupter, PanicOnSenderDrop, PendingTaskKind,
			Task, TaskId, TaskOutput, TaskStatus, TaskWorkState, TaskWorktable,
		},
	},
	TaskRunnerOutput, WorkStealer, WorkerId, ONE_SECOND,
};

const TEN_SECONDS: Duration = Duration::from_secs(10);
const ONE_MINUTE: Duration = Duration::from_secs(60);

const TASK_QUEUE_INITIAL_SIZE: usize = 64;
const PRIORITY_TASK_QUEUE_INITIAL_SIZE: usize = 32;
const ABORT_AND_SUSPEND_MAP_INITIAL_SIZE: usize = 8;

pub(super) enum TaskAddStatus {
	Running,
	Enqueued,
}

struct AbortAndSuspendSignalers {
	abort_tx: oneshot::Sender<oneshot::Sender<Result<(), SystemError>>>,
	suspend_tx: oneshot::Sender<()>,
}

struct RunningTask {
	id: TaskId,
	kind: PendingTaskKind,
	handle: JoinHandle<Result<(), Box<dyn Any + Send>>>,
}

enum WaitingSuspendedTask {
	Task(TaskId),
	None,
}

impl WaitingSuspendedTask {
	const fn is_waiting(&self) -> bool {
		matches!(self, Self::Task(_))
	}
}

pub(super) struct Runner<E: RunError> {
	worker_id: WorkerId,
	system_comm: SystemComm,
	work_stealer: WorkStealer<E>,
	task_kinds: HashMap<TaskId, PendingTaskKind>,
	tasks: VecDeque<TaskWorkState<E>>,
	paused_tasks: HashMap<TaskId, TaskWorkState<E>>,
	suspended_task: Option<TaskWorkState<E>>,
	priority_tasks: VecDeque<TaskWorkState<E>>,
	is_idle: bool,
	waiting_suspension: WaitingSuspendedTask,
	abort_and_suspend_map: HashMap<TaskId, AbortAndSuspendSignalers>,
	stole_task_tx: chan::Sender<Option<StoleTaskMessage<E>>>,
	task_output_tx: chan::Sender<TaskOutputMessage<E>>,
	current_task_handle: Option<RunningTask>,
	suspend_on_shutdown_stole_task_rx: chan::Receiver<Option<StoleTaskMessage<E>>>,
	suspend_on_shutdown_task_output_rx: chan::Receiver<TaskOutputMessage<E>>,
	current_steal_task_handle: Option<JoinHandle<()>>,
	last_steal_attempt_at: Instant,
	steal_attempts_count: u32,
}

type RunnerCreate<E> = (
	Runner<E>,
	chan::Receiver<Option<StoleTaskMessage<E>>>,
	chan::Receiver<TaskOutputMessage<E>>,
);

impl<E: RunError> Runner<E> {
	pub(super) fn new(
		worker_id: WorkerId,
		work_stealer: WorkStealer<E>,
		system_comm: SystemComm,
	) -> RunnerCreate<E> {
		let (stolen_task_tx, stolen_task_rx) = chan::bounded(2);
		let (task_output_tx, task_output_rx) = chan::bounded(8);

		(
			Self {
				worker_id,
				system_comm,
				work_stealer,
				task_kinds: HashMap::with_capacity(TASK_QUEUE_INITIAL_SIZE),
				tasks: VecDeque::with_capacity(TASK_QUEUE_INITIAL_SIZE),
				paused_tasks: HashMap::new(),
				suspended_task: None,
				priority_tasks: VecDeque::with_capacity(PRIORITY_TASK_QUEUE_INITIAL_SIZE),
				is_idle: true,
				waiting_suspension: WaitingSuspendedTask::None,
				abort_and_suspend_map: HashMap::with_capacity(ABORT_AND_SUSPEND_MAP_INITIAL_SIZE),
				stole_task_tx: stolen_task_tx,
				task_output_tx,
				current_task_handle: None,
				suspend_on_shutdown_stole_task_rx: stolen_task_rx.clone(),
				suspend_on_shutdown_task_output_rx: task_output_rx.clone(),
				current_steal_task_handle: None,
				last_steal_attempt_at: Instant::now(),
				steal_attempts_count: 0,
			},
			stolen_task_rx,
			task_output_rx,
		)
	}

	#[instrument(skip(self))]
	pub(super) fn total_tasks(&self) -> usize {
		let priority_tasks_count = self.priority_tasks.len();
		let current_task_count = usize::from(self.current_task_handle.is_some());
		let suspended_task_count = usize::from(self.suspended_task.is_some());
		let tasks_count = self.tasks.len();

		trace!(%priority_tasks_count, %current_task_count, %suspended_task_count, %tasks_count,
			"Tasks count"
		);

		priority_tasks_count + current_task_count + suspended_task_count + tasks_count
	}

	#[instrument(skip(self, task_work_state))]
	pub(super) fn spawn_task_runner(
		&mut self,
		task_id: TaskId,
		task_work_state: TaskWorkState<E>,
	) -> JoinHandle<Result<(), Box<dyn Any + Send>>> {
		let (abort_tx, abort_rx) = oneshot::channel();
		let (suspend_tx, suspend_rx) = oneshot::channel();

		self.abort_and_suspend_map.insert(
			task_id,
			AbortAndSuspendSignalers {
				abort_tx,
				suspend_tx,
			},
		);

		let handle = spawn(
			AssertUnwindSafe(
				run_single_task(
					task_work_state,
					self.task_output_tx.clone(),
					suspend_rx,
					abort_rx,
				)
				.in_current_span(),
			)
			.catch_unwind(),
		);

		trace!("Task runner spawned");

		handle
	}

	#[instrument(skip(self, task_work_state))]
	pub(super) fn new_task(
		&mut self,
		task_id: TaskId,
		task_kind: PendingTaskKind,
		task_work_state: TaskWorkState<E>,
	) {
		trace!("Received new task");

		self.task_kinds.insert(task_id, task_kind);

		match self.inner_add_task(task_id, task_kind, task_work_state) {
			TaskAddStatus::Running => trace!("New task is running"),
			TaskAddStatus::Enqueued => {
				trace!(
					total_tasks = self.total_tasks(),
					"Task enqueued with other tasks"
				);
			}
		}
	}

	#[instrument(skip(self))]
	pub(super) fn resume_task(&mut self, task_id: TaskId) -> Result<(), SystemError> {
		trace!("Resume task request");
		if let Some(task_work_state) = self.paused_tasks.remove(&task_id) {
			task_work_state.worktable.set_unpause();

			match self.inner_add_task(
				task_id,
				*self
					.task_kinds
					.get(&task_id)
					.expect("we added the task kind before pausing it"),
				task_work_state,
			) {
				TaskAddStatus::Running => trace!("Resumed task is running"),
				TaskAddStatus::Enqueued => trace!("Resumed task was enqueued"),
			}

			return Ok(());
		}

		trace!("Task not found");
		Err(SystemError::TaskNotFound(task_id))
	}

	#[instrument(skip(self))]
	pub(super) fn pause_not_running_task(&mut self, task_id: TaskId) -> Result<(), SystemError> {
		if self.paused_tasks.contains_key(&task_id) {
			trace!("Task is already paused");
			return Ok(());
		}

		if let Some(current_task) = &self.current_task_handle {
			if current_task.id == task_id {
				trace!(
					"Task began to run before we managed to pause it, run function will pause it"
				);
				return Ok(()); // The task will pause itself
			}
		}

		if self.pause_suspended_task(task_id) || self.pause_task_from_queues(task_id) {
			return Ok(());
		}

		Err(SystemError::TaskNotFound(task_id))
	}

	#[instrument(skip(self))]
	fn pause_suspended_task(&mut self, task_id: TaskId) -> bool {
		if let Some(suspended_task) = &self.suspended_task {
			if suspended_task.id() == task_id {
				trace!("Task is already suspended but will be paused");

				self.paused_tasks.insert(
					task_id,
					self.suspended_task.take().expect("we just checked it"),
				);

				return true;
			}
		}

		false
	}

	#[instrument(skip(self))]
	fn pause_task_from_queues(&mut self, task_id: TaskId) -> bool {
		if let Some(index) = self
			.priority_tasks
			.iter()
			.position(|task_work_state| task_work_state.id() == task_id)
		{
			self.paused_tasks.insert(
				task_id,
				self.priority_tasks
					.remove(index)
					.expect("we just checked it"),
			);

			return true;
		}

		if let Some(index) = self
			.tasks
			.iter()
			.position(|task_work_state| task_work_state.id() == task_id)
		{
			self.paused_tasks.insert(
				task_id,
				self.tasks.remove(index).expect("we just checked it"),
			);

			return true;
		}

		false
	}

	#[instrument(skip(self))]
	pub(super) fn cancel_not_running_task(&mut self, task_id: &TaskId) -> Result<(), SystemError> {
		trace!("Cancel not running task request");

		if let Some(current_task) = &self.current_task_handle {
			if current_task.id == *task_id {
				trace!(
					"Task began to run before we managed to cancel it, run function will cancel it"
				);
				return Ok(()); // The task will cancel itself
			}
		}

		// We only remove from task_kinds as if the task is already running, it will be removed when we
		// process its cancelled output later
		self.task_kinds.remove(task_id);

		if let Some(suspended_task) = &self.suspended_task {
			if suspended_task.id() == *task_id {
				trace!("Task is already suspended but will be canceled");

				send_cancel_task_response(self.suspended_task.take().expect("we just checked it"));

				return Ok(());
			}
		}

		if self.cancel_task_from_queues(task_id) {
			return Ok(());
		}

		Err(SystemError::TaskNotFound(*task_id))

		// If the task is not found, then it's possible that the user already canceled it but still have the handle
	}

	#[instrument(skip(self))]
	#[inline]
	fn cancel_task_from_queues(&mut self, task_id: &TaskId) -> bool {
		if let Some(index) = self
			.priority_tasks
			.iter()
			.position(|task_work_state| task_work_state.id() == *task_id)
		{
			send_cancel_task_response(
				self.priority_tasks
					.remove(index)
					.expect("we just checked it"),
			);

			return true;
		}

		if let Some(index) = self
			.tasks
			.iter()
			.position(|task_work_state| task_work_state.id() == *task_id)
		{
			send_cancel_task_response(self.tasks.remove(index).expect("we just checked it"));

			return true;
		}

		if let Some(task_work_state) = self.paused_tasks.remove(task_id) {
			send_cancel_task_response(task_work_state);

			return true;
		}

		false
	}

	#[instrument(skip(self, task_work_state))]
	#[inline]
	fn add_task_when_idle(
		&mut self,
		task_id: TaskId,
		task_kind: PendingTaskKind,
		task_work_state: TaskWorkState<E>,
	) {
		trace!("Idle worker will process the new task");
		let handle = self.spawn_task_runner(task_id, task_work_state);

		self.current_task_handle = Some(RunningTask {
			id: task_id,
			kind: task_kind,
			handle,
		});

		// Doesn't need to report working back to system as it already registered
		// that we're not idle anymore when it dispatched the task to this worker
		self.is_idle = false;
	}

	#[instrument(skip(self, task_work_state))]
	#[inline]
	fn add_task_when_busy(
		&mut self,
		new_kind: PendingTaskKind,
		task_work_state: TaskWorkState<E>,
		old_task_id: TaskId,
		old_kind: PendingTaskKind,
	) -> TaskAddStatus {
		match (new_kind, old_kind) {
			(PendingTaskKind::Priority, PendingTaskKind::Priority) => {
				trace!("Old and new tasks have priority, will put new task on priority queue");
				self.priority_tasks.push_front(task_work_state);
				TaskAddStatus::Enqueued
			}
			(PendingTaskKind::Priority, PendingTaskKind::Normal) => {
				if self.waiting_suspension.is_waiting() {
					trace!(
						"Worker is already waiting for a task to be suspended, will enqueue new task"
					);
					self.priority_tasks.push_front(task_work_state);
				} else {
					trace!("Old task will be suspended");
					// We put the query at the top of the priority queue, so it will be
					// dispatched by the run function as soon as the current task is suspended
					self.priority_tasks.push_front(task_work_state);

					if self
						.abort_and_suspend_map
						.remove(&old_task_id)
						.expect("we always store the abort and suspend signalers")
						.suspend_tx
						.send(())
						.is_err()
					{
						warn!(%old_task_id,
							"Suspend channel closed before receiving suspend signal. \
							This probably happened because the task finished before we could suspend it."
						);
					}

					self.waiting_suspension = WaitingSuspendedTask::Task(old_task_id);
				}

				TaskAddStatus::Running
			}
			(_, _) => {
				trace!("New task doesn't have priority and will be enqueued");
				self.tasks.push_back(task_work_state);

				TaskAddStatus::Enqueued
			}
		}
	}

	#[instrument(skip(self, task_work_state))]
	#[inline]
	pub(super) fn inner_add_task(
		&mut self,
		task_id: TaskId,
		task_kind: PendingTaskKind,
		task_work_state: TaskWorkState<E>,
	) -> TaskAddStatus {
		if self.is_idle {
			self.add_task_when_idle(task_id, task_kind, task_work_state);
			TaskAddStatus::Running
		} else {
			trace!("Worker is busy");

			let RunningTask {
				id: old_task_id,
				kind: old_kind,
				..
			} = self
				.current_task_handle
				.as_ref()
				.expect("Worker isn't idle, but no task is running");

			self.add_task_when_busy(task_kind, task_work_state, *old_task_id, *old_kind)
		}
	}

	#[instrument(skip(self))]
	pub(super) async fn force_task_abortion(
		&mut self,
		task_id: &TaskId,
	) -> Result<(), SystemError> {
		if let Some(AbortAndSuspendSignalers { abort_tx, .. }) =
			self.abort_and_suspend_map.remove(task_id)
		{
			let (tx, rx) = oneshot::channel();

			if abort_tx.send(tx).is_err() {
				debug!(
					"Failed to send force abortion request, \
					the task probably finished before we could abort it"
				);

				Ok(())
			} else {
				match timeout(ONE_SECOND, rx).await {
					Ok(Ok(res)) => res,
					// If the sender was dropped, then the task finished before we could
					// abort it which is fine
					Ok(Err(_)) => Ok(()),
					Err(_) => Err(SystemError::TaskForcedAbortTimeout(*task_id)),
				}
			}
		} else {
			trace!("Forced abortion of a not running task request");

			if let Some(current_task) = &self.current_task_handle {
				if current_task.id == *task_id {
					trace!(
						"Task began to run before we managed to abort it, \
						run function will abort it"
					);
					return Ok(()); // The task will abort itself
				}
			}

			self.task_kinds.remove(task_id);

			if let Some(suspended_task) = &self.suspended_task {
				if suspended_task.id() == *task_id {
					trace!("Task is already suspended but will be force aborted");

					send_forced_abortion_task_response(
						self.suspended_task.take().expect("we just checked it"),
					);

					return Ok(());
				}
			}

			if let Some(index) = self
				.priority_tasks
				.iter()
				.position(|task_work_state| task_work_state.id() == *task_id)
			{
				send_forced_abortion_task_response(
					self.priority_tasks
						.remove(index)
						.expect("we just checked it"),
				);

				return Ok(());
			}

			if let Some(index) = self
				.tasks
				.iter()
				.position(|task_work_state| task_work_state.id() == *task_id)
			{
				send_forced_abortion_task_response(
					self.tasks.remove(index).expect("we just checked it"),
				);

				return Ok(());
			}

			// If the task is not found, then it's possible that
			// the user already aborted it but still have the handle
			Ok(())
		}
	}

	#[instrument(skip(self, tx))]
	pub(super) async fn shutdown(mut self, tx: oneshot::Sender<()>) {
		trace!("Worker beginning shutdown process");

		trace!("Aborting steal task for shutdown if there is one running");

		self.abort_steal_task();

		let Self {
			tasks,
			suspended_task,
			paused_tasks,
			priority_tasks,
			is_idle,
			abort_and_suspend_map,
			stole_task_tx: stolen_task_tx,
			task_output_tx,
			mut current_task_handle,
			suspend_on_shutdown_stole_task_rx,
			suspend_on_shutdown_task_output_rx,
			..
		} = self;

		if is_idle {
			trace!("Worker is idle, no tasks to shutdown");
			assert!(
				current_task_handle.is_none(),
				"can't shutdown with a running task if we're idle"
			);
			assert!(
				tasks.is_empty(),
				"can't shutdown with pending tasks if we're idle"
			);
			assert!(
				priority_tasks.is_empty(),
				"can't shutdown with priority tasks if we're idle"
			);
			assert!(
				suspended_task.is_none(),
				"can't shutdown with a suspended task if we're idle"
			);

			paused_tasks
				.into_values()
				.for_each(send_shutdown_task_response);
		} else {
			trace!("Worker is busy, will shutdown tasks");

			if let Some(RunningTask {
				id: task_id,
				handle,
				..
			}) = current_task_handle.take()
			{
				for (task_id, AbortAndSuspendSignalers { suspend_tx, .. }) in abort_and_suspend_map
				{
					if suspend_tx.send(()).is_err() {
						warn!(%task_id,
							"Shutdown request channel closed before sending abort signal"
						);
					} else {
						trace!(%task_id, "Sent suspend signal for task on shutdown");
					}
				}

				match handle.await {
					Ok(Ok(())) => { /* Everything is Awesome! */ }
					Ok(Err(_)) => {
						error!(%task_id, "Task panicked");
					}
					Err(e) => {
						error!(%task_id, ?e, "Task failed to join");
					}
				}

				stolen_task_tx.close();
				task_output_tx.close();

				Self::process_tasks_being_suspended_on_shutdown(
					suspend_on_shutdown_stole_task_rx,
					suspend_on_shutdown_task_output_rx,
				)
				.await;
			}

			priority_tasks
				.into_iter()
				.chain(suspended_task.into_iter())
				.chain(paused_tasks.into_values())
				.chain(tasks.into_iter())
				.for_each(send_shutdown_task_response);
		}

		trace!("Worker shutdown process completed");

		if tx.send(()).is_err() {
			warn!("Shutdown request channel closed before sending ack");
		}
	}

	async fn process_tasks_being_suspended_on_shutdown(
		suspend_on_shutdown_stole_task_rx: chan::Receiver<Option<StoleTaskMessage<E>>>,
		suspend_on_shutdown_task_output_rx: chan::Receiver<TaskOutputMessage<E>>,
	) {
		enum StreamMessage<E: RunError> {
			Output(TaskOutputMessage<E>),
			Steal(Option<StoleTaskMessage<E>>),
		}

		let mut msg_stream = pin!((
			suspend_on_shutdown_stole_task_rx.map(StreamMessage::Steal),
			suspend_on_shutdown_task_output_rx.map(StreamMessage::Output),
		)
			.merge());

		while let Some(msg) = msg_stream.next().await {
			match msg {
				StreamMessage::Output(TaskOutputMessage(task_id, res)) => match res {
					Ok(TaskRunnerOutput {
						task_work_state,
						status,
					}) => match status {
						InternalTaskExecStatus::Done(out) => {
							send_complete_task_response(task_work_state, out);
						}

						InternalTaskExecStatus::Canceled => {
							send_cancel_task_response(task_work_state);
						}

						InternalTaskExecStatus::Suspend | InternalTaskExecStatus::Paused => {
							send_shutdown_task_response(task_work_state);
						}

						InternalTaskExecStatus::Error(e) => {
							send_error_task_response(task_work_state, e);
						}
					},
					Err(()) => {
						error!(%task_id, "Task failed to suspend on shutdown");
					}
				},

				StreamMessage::Steal(Some(StoleTaskMessage(task_work_state))) => {
					trace!(
						task_id = %task_work_state.id(),
						"Stole task",
					);

					send_shutdown_task_response(task_work_state);
				}

				StreamMessage::Steal(None) => {}
			}
		}
	}

	pub(super) fn get_next_task(&mut self) -> Option<(PendingTaskKind, TaskWorkState<E>)> {
		if let Some(task) = self.priority_tasks.pop_front() {
			return Some((PendingTaskKind::Priority, task));
		}

		if let Some(task) = self.suspended_task.take() {
			task.worktable.set_unpause();
			return Some((PendingTaskKind::Suspended, task));
		}

		self.tasks
			.pop_front()
			.map(|task| (PendingTaskKind::Normal, task))
	}

	#[instrument(skip_all)]
	pub(super) async fn steal_request(
		&mut self,
		stealer_id: WorkerId,
		stolen_task_tx: chan::Sender<Option<StoleTaskMessage<E>>>,
	) -> bool {
		while let Some((kind, task_work_state)) = self.get_next_task() {
			let task_id = task_work_state.id();
			self.task_kinds.remove(&task_id);

			trace!(%task_id, ?kind, "Task being stolen");

			if task_work_state.worktable.has_canceled() {
				trace!(%task_id, "Task was canceled before we could steal it");
				send_cancel_task_response(task_work_state);
				continue;
			}

			if task_work_state.worktable.has_aborted() {
				trace!(%task_id, "Task was force aborted before we could steal it");
				send_forced_abortion_task_response(task_work_state);
				continue;
			}

			if task_work_state.worktable.is_paused() {
				trace!(%task_id, "Task was paused before we could steal it");
				self.task_kinds.insert(task_id, kind);
				self.paused_tasks.insert(task_id, task_work_state);
				continue;
			}

			trace!(%task_id, ?kind, "Task being stolen");

			task_work_state.worktable.change_worker(stealer_id);

			if let Err(chan::SendError(Some(StoleTaskMessage(task_work_state)))) = stolen_task_tx
				.send(Some(StoleTaskMessage(task_work_state)))
				.await
			{
				warn!("Steal request channel closed before sending task");
				task_work_state.worktable.change_worker(self.worker_id);
				match kind {
					PendingTaskKind::Normal => self.tasks.push_front(task_work_state),
					PendingTaskKind::Priority => self.priority_tasks.push_front(task_work_state),
					PendingTaskKind::Suspended => {
						assert!(
							self.suspended_task.is_none(),
							"tried to suspend a task when we already have a suspended task"
						);
						self.suspended_task = Some(task_work_state);
					}
				}

				self.task_kinds.insert(task_id, kind);

				return false;
			}

			return true; // Successfully stole the task
		}

		false // No task to steal
	}

	#[instrument(skip(self))]
	#[inline]
	pub(super) async fn dispatch_next_task(&mut self, finished_task_id: &TaskId) {
		self.abort_and_suspend_map.remove(finished_task_id);

		let RunningTask {
			id: old_task_id,

			handle,
			..
		} = self
			.current_task_handle
			.take()
			.expect("Task handle missing, but task output received");

		assert_eq!(*finished_task_id, old_task_id, "Task output id mismatch"); // Sanity check

		match handle.await {
			Ok(Ok(())) => { /* Everything is Awesome! */ }
			Ok(Err(_)) => {
				error!("Task panicked");
			}
			Err(e) => {
				error!(?e, "Task failed to join");
			}
		}

		if let Some((next_task_kind, task_work_state)) = self.get_next_task() {
			let next_task_id = task_work_state.id();

			trace!(%next_task_id, ?next_task_kind, "Dispatching next task");

			let handle = self.spawn_task_runner(next_task_id, task_work_state);

			self.current_task_handle = Some(RunningTask {
				id: next_task_id,
				kind: next_task_kind,
				handle,
			});
		} else {
			self.is_idle = true;
			self.system_comm.idle_report(self.worker_id);

			if self.current_steal_task_handle.is_none() {
				self.current_steal_task_handle = Some(dispatch_steal_request(
					self.worker_id,
					self.work_stealer.clone(),
					self.stole_task_tx.clone(),
				));
			}
		}
	}

	#[instrument(skip(self, task_work_state, status))]
	pub(super) async fn process_task_output(
		&mut self,
		task_id: &TaskId,
		TaskRunnerOutput {
			task_work_state,
			status,
		}: TaskRunnerOutput<E>,
	) {
		match status {
			InternalTaskExecStatus::Done(out) => {
				self.task_kinds.remove(task_id);
				send_complete_task_response(task_work_state, out);
			}

			InternalTaskExecStatus::Paused => {
				self.paused_tasks.insert(*task_id, task_work_state);
				trace!("Task paused");
			}

			InternalTaskExecStatus::Canceled => {
				self.task_kinds.remove(task_id);
				send_cancel_task_response(task_work_state);
			}

			InternalTaskExecStatus::Error(e) => {
				self.task_kinds.remove(task_id);
				send_error_task_response(task_work_state, e);
			}

			InternalTaskExecStatus::Suspend => {
				assert!(
					self.suspended_task.is_none(),
					"tried to suspend a task when we already have a suspended task"
				);
				self.suspended_task = Some(task_work_state);
				trace!("Task suspended");

				self.clean_suspended_task(task_id);
			}
		}

		self.dispatch_next_task(task_id).await;
	}

	#[instrument(skip(self))]
	pub(super) fn idle_check(&mut self) {
		if self.is_idle {
			if self.current_steal_task_handle.is_none() {
				self.steal_attempt();
			}

			self.idle_memory_cleanup();
		}
	}

	#[instrument(skip(self), fields(steal_attempts_count = self.steal_attempts_count))]
	fn steal_attempt(&mut self) {
		let elapsed = self.last_steal_attempt_at.elapsed();
		let required = (TEN_SECONDS * self.steal_attempts_count).min(ONE_MINUTE);

		if elapsed > required {
			self.current_steal_task_handle = Some(dispatch_steal_request(
				self.worker_id,
				self.work_stealer.clone(),
				self.stole_task_tx.clone(),
			));
			self.last_steal_attempt_at = Instant::now();
		}
	}

	fn idle_memory_cleanup(&mut self) {
		// As we're idle, let's check if we need to do some memory cleanup
		if self.tasks.capacity() > TASK_QUEUE_INITIAL_SIZE {
			assert_eq!(self.tasks.len(), 0);
			self.tasks.shrink_to(TASK_QUEUE_INITIAL_SIZE);
		}

		if self.task_kinds.capacity() > TASK_QUEUE_INITIAL_SIZE {
			assert_eq!(
				self.task_kinds.len(),
				self.paused_tasks.len(),
				"If we're idle, the number of task_kinds MUST be equal to the number of paused tasks"
			);
			self.task_kinds.shrink_to(TASK_QUEUE_INITIAL_SIZE);
		}

		if self.priority_tasks.capacity() > PRIORITY_TASK_QUEUE_INITIAL_SIZE {
			assert_eq!(self.priority_tasks.len(), 0);
			self.priority_tasks
				.shrink_to(PRIORITY_TASK_QUEUE_INITIAL_SIZE);
		}

		if self.paused_tasks.capacity() != self.paused_tasks.len() {
			self.paused_tasks.shrink_to_fit();
		}

		if self.abort_and_suspend_map.capacity() > ABORT_AND_SUSPEND_MAP_INITIAL_SIZE {
			assert!(self.abort_and_suspend_map.len() < ABORT_AND_SUSPEND_MAP_INITIAL_SIZE);
			self.abort_and_suspend_map
				.shrink_to(ABORT_AND_SUSPEND_MAP_INITIAL_SIZE);
		}
	}

	#[instrument(skip(self))]
	pub(super) fn abort_steal_task(&mut self) {
		if let Some(steal_task_handle) = self.current_steal_task_handle.take() {
			steal_task_handle.abort();
			trace!("Aborted steal task");
		}
	}

	#[instrument(
		skip(self, maybe_new_task),
		fields(
			maybe_new_task = ?maybe_new_task.as_ref()
				.map(|StoleTaskMessage(task_work_state)| task_work_state.id())
		)
	)]
	pub(super) async fn process_stolen_task(
		&mut self,
		maybe_new_task: Option<StoleTaskMessage<E>>,
	) {
		if let Some(steal_task_handle) = self.current_steal_task_handle.take() {
			if let Err(e) = steal_task_handle.await {
				error!(?e, "Steal task failed to join");
			}
		}

		if let Some(StoleTaskMessage(task_work_state)) = maybe_new_task {
			self.system_comm.working_report(self.worker_id);

			let stolen_task_id = task_work_state.id();

			trace!(%stolen_task_id, "Stolen task");

			self.steal_attempts_count = 0;
			self.new_task(stolen_task_id, task_work_state.kind(), task_work_state);
		} else {
			self.steal_attempts_count += 1;
		}
	}

	#[instrument(skip(self))]
	pub(crate) fn clean_suspended_task(&mut self, task_id: &TaskId) {
		match self.waiting_suspension {
			WaitingSuspendedTask::Task(waiting_task_id) if waiting_task_id == *task_id => {
				trace!("Task was suspended and will be cleaned");
				self.waiting_suspension = WaitingSuspendedTask::None;
			}
			WaitingSuspendedTask::Task(_) => {
				trace!("Task wasn't suspended, ignoring");
			}
			WaitingSuspendedTask::None => {
				// Everything is Awesome!
			}
		}
	}

	#[instrument(skip(self))]
	pub(crate) async fn clear_errored_task(&mut self, task_id: TaskId) {
		self.task_kinds.remove(&task_id);

		self.clean_suspended_task(&task_id);

		trace!("Cleansed errored task");

		self.dispatch_next_task(&task_id).await;
	}
}

type RunTaskOutput<E> = (Box<dyn Task<E>>, Result<Result<ExecStatus, E>, SystemError>);

#[instrument(skip(task, worktable, interrupter))]
fn handle_run_task_attempt<E: RunError>(
	task_id: TaskId,
	mut task: Box<dyn Task<E>>,
	worktable: &TaskWorktable,
	interrupter: Arc<Interrupter>,
) -> JoinHandle<RunTaskOutput<E>> {
	spawn({
		let already_paused = worktable.is_paused();
		let already_canceled = worktable.has_canceled();
		let already_aborted = worktable.has_aborted();

		let early_result = if already_paused {
			trace!("Task was paused before running");

			Some(Ok(Ok(ExecStatus::Paused)))
		} else if already_canceled {
			trace!("Task was canceled before running");

			Some(Ok(Ok(ExecStatus::Canceled)))
		} else if already_aborted {
			trace!("Task was aborted before running");

			Some(Err(SystemError::TaskAborted(task_id)))
		} else {
			// We can mark that the task has actually started now
			worktable.set_started();
			None
		};

		async move {
			if let Some(res) = early_result {
				(task, res)
			} else {
				let run_result = if let Some(timeout_duration) = task.with_timeout() {
					(task.run(&interrupter).map(Ok), async move {
						sleep(timeout_duration)
							.map(|()| Err(SystemError::TaskTimeout(task_id)))
							.await
					})
						.race()
						.await
				} else {
					task.run(&interrupter).map(Ok).await
				};

				match run_result {
					Ok(res) => {
						trace!(?res, "Ran task");

						(task, Ok(res))
					}
					Err(e) => (task, Err(e)),
				}
			}
		}
		.in_current_span()
	})
}

fn handle_task_suspension(
	has_suspended: Arc<AtomicBool>,
	worktable: Arc<TaskWorktable>,
	suspend_rx: oneshot::Receiver<()>,
) -> JoinHandle<()> {
	spawn(
		async move {
			if suspend_rx.await.is_ok() {
				let (tx, rx) = oneshot::channel();

				trace!("Suspend signal received");

				worktable.suspend(tx, has_suspended);

				if rx.await.is_ok() {
					trace!("Suspending");
				} else {
					// The task probably finished before we could suspend it so the channel was dropped
					trace!("Suspend channel closed");
				}
			} else {
				trace!("Suspend channel closed, task probably finished before we could suspend it");
			}
		}
		.in_current_span(),
	)
}

type PartialTaskWorkState<E> = (
	TaskId,
	Arc<TaskWorktable>,
	PanicOnSenderDrop<E>,
	Arc<Interrupter>,
);

async fn emit_task_completed_message<E: RunError>(
	run_task_output: RunTaskOutput<E>,
	has_suspended: Arc<AtomicBool>,
	(task_id, worktable, done_tx, interrupter): PartialTaskWorkState<E>,
	task_output_tx: chan::Sender<TaskOutputMessage<E>>,
) {
	match run_task_output {
		(task, Ok(res)) => {
			trace!(?res, "Task completed ok");

			task_output_tx
				.send(TaskOutputMessage(task_id, {
					let mut internal_status = res.into();
					let suspended = has_suspended.load(Ordering::SeqCst);

					match internal_status {
						InternalTaskExecStatus::Paused if suspended => {
							internal_status = InternalTaskExecStatus::Suspend;
						}

						InternalTaskExecStatus::Paused | InternalTaskExecStatus::Suspend => {
							/* Nothing to do */
						}

						InternalTaskExecStatus::Done(_)
						| InternalTaskExecStatus::Canceled
						| InternalTaskExecStatus::Error(_) => {
							trace!(?internal_status, "Task completed, closing interrupter");
							interrupter.close();
						}
					}

					Ok(TaskRunnerOutput {
						task_work_state: TaskWorkState {
							task,
							worktable,
							done_tx,
							interrupter,
						},
						status: internal_status,
					})
				}))
				.await
				.expect("Task runner channel closed while sending task output");
		}

		(_, Err(e)) => {
			error!(?e, "Task had an error");

			if done_tx
				.send(if matches!(e, SystemError::TaskAborted(_)) {
					worktable.set_aborted();
					Ok(TaskStatus::ForcedAbortion)
				} else {
					worktable.set_failed();
					Err(e)
				})
				.is_err()
			{
				error!("Task done channel closed while sending error response");
			}

			task_output_tx
				.send(TaskOutputMessage(task_id, Err(())))
				.await
				.expect("Task runner channel closed while sending task output");
		}
	}
}

#[instrument(skip_all, fields(task_id = %task.id()))]
async fn run_single_task<E: RunError>(
	TaskWorkState {
		task,
		worktable,
		interrupter,
		done_tx,
	}: TaskWorkState<E>,
	task_output_tx: chan::Sender<TaskOutputMessage<E>>,
	suspend_rx: oneshot::Receiver<()>,
	abort_rx: oneshot::Receiver<oneshot::Sender<Result<(), SystemError>>>,
) {
	enum RaceOutput<E: RunError> {
		Completed(Result<RunTaskOutput<E>, JoinError>),
		Abort(oneshot::Sender<Result<(), SystemError>>),
	}

	let task_id = task.id();

	trace!("Running task");

	let handle = handle_run_task_attempt(task_id, task, &worktable, Arc::clone(&interrupter));

	let task_abort_handle = handle.abort_handle();

	let has_suspended = Arc::new(AtomicBool::new(false));

	let suspender_handle = handle_task_suspension(
		Arc::clone(&has_suspended),
		Arc::clone(&worktable),
		suspend_rx,
	);

	match (async { RaceOutput::Completed(handle.await) }, async move {
		if let Ok(tx) = abort_rx.await {
			trace!("Aborting task");
			RaceOutput::Abort(tx)
		} else {
			// If the abort channel is closed, we should just ignore it and keep waiting for the task to finish
			// as we're being suspended by the worker
			trace!("Abort channel closed, will wait for task to finish");
			pending().await
		}
	})
		.race()
		.await
	{
		RaceOutput::Completed(Ok(run_task_output)) => {
			emit_task_completed_message(
				run_task_output,
				has_suspended,
				(task_id, worktable, done_tx, interrupter),
				task_output_tx,
			)
			.await;
		}

		RaceOutput::Completed(Err(join_error)) => {
			interrupter.close();
			error!(?join_error, "Task failed to join");
			if done_tx.send(Err(SystemError::TaskJoin(task_id))).is_err() {
				error!("Task done channel closed while sending join error response");
			}

			worktable.set_failed();

			if task_output_tx
				.send(TaskOutputMessage(task_id, Err(())))
				.await
				.is_err()
			{
				error!("Task runner channel closed while sending join error response");
			}
		}

		RaceOutput::Abort(tx) => {
			task_abort_handle.abort();

			trace!("Task aborted");

			if done_tx.send(Ok(TaskStatus::ForcedAbortion)).is_err() {
				error!("Task done channel closed while sending abort error response");
			}

			worktable.set_aborted();

			if task_output_tx
				.send(TaskOutputMessage(task_id, Err(())))
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
		// if we received a suspend signal this abort will do nothing, as the task finished already
		suspender_handle.abort();
	}
}

#[instrument(skip(task, done_tx, worktable, out), fields(task_id = %task.id()))]
fn send_complete_task_response<E: RunError>(
	TaskWorkState {
		done_tx,
		worktable,
		task,
		..
	}: TaskWorkState<E>,
	out: TaskOutput,
) {
	worktable.set_completed();
	worktable.set_finalized();
	if done_tx
		.send(Ok(TaskStatus::Done((task.id(), out))))
		.is_err()
	{
		warn!("Task done channel closed before sending done response for task");
	} else {
		trace!("Emitted task done signal on task completion");
	}
}

#[instrument(skip(task, done_tx, worktable), fields(task_id = %task.id()))]
fn send_cancel_task_response<E: RunError>(
	TaskWorkState {
		task,
		done_tx,
		worktable,
		..
	}: TaskWorkState<E>,
) {
	worktable.set_canceled();
	worktable.set_finalized();
	if done_tx.send(Ok(TaskStatus::Canceled)).is_err() {
		warn!("Task done channel closed before sending canceled response for task");
	} else {
		trace!("Emitted task canceled signal on cancel request");
	}
}

#[instrument(skip(task, done_tx, worktable), fields(task_id = %task.id()))]
fn send_shutdown_task_response<E: RunError>(
	TaskWorkState {
		task,
		done_tx,
		worktable,
		..
	}: TaskWorkState<E>,
) {
	worktable.set_shutdown();
	worktable.set_finalized();
	if done_tx.send(Ok(TaskStatus::Shutdown(task))).is_err() {
		warn!("Task done channel closed before sending shutdown response for task");
	} else {
		trace!("Successfully suspended and sent back DynTask on worker shutdown");
	}
}

#[instrument(skip(task, done_tx, worktable), fields(task_id = %task.id()))]
fn send_error_task_response<E: RunError>(
	TaskWorkState {
		task,
		done_tx,
		worktable,
		..
	}: TaskWorkState<E>,
	e: E,
) {
	worktable.set_completed();
	worktable.set_finalized();
	if done_tx.send(Ok(TaskStatus::Error(e))).is_err() {
		warn!("Task done channel closed before sending error response for task");
	} else {
		trace!("Emitted task error signal");
	}
}

#[instrument(skip(task, done_tx, worktable), fields(task_id = %task.id()))]
fn send_forced_abortion_task_response<E: RunError>(
	TaskWorkState {
		task,
		done_tx,
		worktable,
		..
	}: TaskWorkState<E>,
) {
	worktable.set_aborted();
	worktable.set_finalized();
	if done_tx.send(Ok(TaskStatus::ForcedAbortion)).is_err() {
		warn!("Task done channel closed before sending forced abortion response for task");
	} else {
		trace!("Emitted task forced abortion signal");
	}
}

fn dispatch_steal_request<E: RunError>(
	worker_id: WorkerId,
	work_stealer: WorkStealer<E>,
	stole_task_tx: chan::Sender<Option<StoleTaskMessage<E>>>,
) -> JoinHandle<()> {
	spawn(async move { work_stealer.steal(worker_id, &stole_task_tx).await }.in_current_span())
}
