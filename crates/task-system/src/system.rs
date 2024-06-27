use std::{
	cell::RefCell,
	collections::HashSet,
	fmt,
	future::Future,
	num::NonZeroUsize,
	pin::pin,
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
};

use async_channel as chan;
use futures::StreamExt;
use futures_concurrency::future::Join;
use tokio::{spawn, sync::oneshot, task::JoinHandle};
use tracing::{error, info, instrument, trace, warn, Instrument};

use super::{
	error::{DispatcherShutdownError, RunError, SystemError},
	message::SystemMessage,
	task::{IntoTask, Task, TaskHandle, TaskId, TaskWorktable},
	worker::{AtomicWorkerId, WorkStealer, Worker, WorkerBuilder},
};

/// The task system is the main entry point for the library, it is responsible for creating and managing the workers
/// and dispatching tasks to them.
///
/// It also provides a way to shutdown the system returning all pending and running tasks.
/// It uses internal mutability so it can be shared without hassles using [`Arc`].
pub struct System<E: RunError> {
	workers: Arc<Vec<Worker<E>>>,
	msgs_tx: chan::Sender<SystemMessage>,
	dispatcher: BaseDispatcher<E>,
	handle: RefCell<Option<JoinHandle<()>>>,
	has_shutdown: Arc<AtomicBool>,
}

impl<E: RunError> System<E> {
	/// Created a new task system with a number of workers equal to the available parallelism in the user's machine.
	pub fn new() -> Self {
		// TODO: Using only the half of available cores, make this configurable on runtime in the future
		let workers_count = usize::max(
			std::thread::available_parallelism().map_or_else(
				|e| {
					error!(?e, "Failed to get available parallelism in the job system");
					1
				},
				NonZeroUsize::get,
			) / 2,
			1,
		);

		let (msgs_tx, msgs_rx) = chan::bounded(8);
		let system_comm = SystemComm(msgs_tx.clone());

		let (workers_builders, worker_comms) = (0..workers_count)
			.map(WorkerBuilder::new)
			.unzip::<_, _, Vec<_>, Vec<_>>();

		let task_stealer = WorkStealer::new(worker_comms);

		let idle_workers = Arc::new((0..workers_count).map(|_| AtomicBool::new(true)).collect());

		let workers = Arc::new(
			workers_builders
				.into_iter()
				.map(|builder| builder.build(system_comm.clone(), task_stealer.clone()))
				.collect::<Vec<_>>(),
		);

		let handle = spawn({
			let workers = Arc::clone(&workers);
			let idle_workers = Arc::clone(&idle_workers);

			async move {
				trace!("Task System message processing task starting...");
				while let Err(e) = spawn(Self::run(
					Arc::clone(&workers),
					Arc::clone(&idle_workers),
					msgs_rx.clone(),
				))
				.await
				{
					if e.is_panic() {
						error!(?e, "Task system panicked");
					} else {
						trace!("Task system received shutdown signal and will exit...");
						break;
					}
					trace!("Restarting task system message processing task...");
				}

				info!("Task system gracefully shutdown");
			}
		});

		info!(%workers_count, "Task system online!");

		let has_shutdown = Arc::new(AtomicBool::new(false));

		Self {
			workers: Arc::clone(&workers),
			msgs_tx,
			dispatcher: BaseDispatcher {
				workers,
				idle_workers,
				last_worker_id: Arc::new(AtomicWorkerId::new(0)),
				has_shutdown: Arc::clone(&has_shutdown),
			},
			handle: RefCell::new(Some(handle)),
			has_shutdown,
		}
	}

	/// Returns the number of workers in the system.
	pub fn workers_count(&self) -> usize {
		self.workers.len()
	}

	/// Dispatches a task to the system, the task will be assigned to a worker and executed as soon as possible.
	#[allow(clippy::missing_panics_doc)]
	pub async fn dispatch(
		&self,
		into_task: impl IntoTask<E>,
	) -> Result<TaskHandle<E>, DispatcherShutdownError<E>> {
		self.dispatcher.dispatch(into_task).await
	}

	/// Dispatches many tasks to the system, the tasks will be assigned to workers and executed as soon as possible.
	#[allow(clippy::missing_panics_doc)]
	pub async fn dispatch_many<I: IntoIterator<Item = impl IntoTask<E>> + Send>(
		&self,
		into_tasks: I,
	) -> Result<Vec<TaskHandle<E>>, DispatcherShutdownError<E>>
	where
		<I as IntoIterator>::IntoIter: Send,
	{
		self.dispatcher.dispatch_many(into_tasks).await
	}

	/// Returns a dispatcher that can be used to remotely dispatch tasks to the system.
	pub fn get_dispatcher(&self) -> BaseDispatcher<E> {
		self.dispatcher.clone()
	}

	async fn run(
		workers: Arc<Vec<Worker<E>>>,
		idle_workers: Arc<Vec<AtomicBool>>,
		msgs_rx: chan::Receiver<SystemMessage>,
	) {
		let mut msg_stream = pin!(msgs_rx);

		while let Some(msg) = msg_stream.next().await {
			match msg {
				SystemMessage::IdleReport(worker_id) => {
					idle_workers[worker_id].store(true, Ordering::Relaxed);
				}

				SystemMessage::WorkingReport(worker_id) => {
					idle_workers[worker_id].store(false, Ordering::Relaxed);
				}

				SystemMessage::ResumeTask {
					task_id,
					task_work_table,
					ack,
				} => dispatch_resume_request(&workers, task_id, task_work_table, ack),

				SystemMessage::PauseNotRunningTask {
					task_id,
					task_work_table,
					ack,
				} => {
					dispatch_pause_not_running_task_request(
						&workers,
						task_id,
						task_work_table,
						ack,
					);
				}

				SystemMessage::CancelNotRunningTask {
					task_id,
					task_work_table,
					ack,
				} => dispatch_cancel_not_running_task_request(
					&workers,
					task_id,
					task_work_table,
					ack,
				),

				SystemMessage::ForceAbortion {
					task_id,
					task_work_table,
					ack,
				} => dispatch_force_abortion_task_request(&workers, task_id, task_work_table, ack),

				SystemMessage::ShutdownRequest(tx) => {
					tx.send(Ok(()))
						.expect("System channel closed trying to shutdown");
					return;
				}
			}
		}
	}

	/// Shuts down the system, returning all pending and running tasks to their respective handles.
	///
	/// # Panics
	///
	/// If the system message channel is closed for some unknown reason or if we fail to respond to
	/// oneshot channel with shutdown response.
	pub async fn shutdown(&self) {
		self.has_shutdown.store(true, Ordering::Release);
		if let Some(handle) = self
			.handle
			.try_borrow_mut()
			.ok()
			.and_then(|mut maybe_handle| maybe_handle.take())
		{
			self.workers
				.iter()
				.map(|worker| async move { worker.shutdown().await })
				.collect::<Vec<_>>()
				.join()
				.await;

			let (tx, rx) = oneshot::channel();

			self.msgs_tx
				.send(SystemMessage::ShutdownRequest(tx))
				.await
				.expect("Task system channel closed trying to shutdown");

			if let Err(e) = rx
				.await
				.expect("Task system channel closed trying to shutdown")
			{
				error!("Task system failed to shutdown: {e:#?}");
			}

			if let Err(e) = handle.await {
				error!(?e, "Task system failed to shutdown on handle await");
			}
		} else {
			warn!("Trying to shutdown the tasks system that was already shutdown");
		}
	}
}

#[instrument(skip(workers, ack))]
fn dispatch_resume_request<E: RunError>(
	workers: &Arc<Vec<Worker<E>>>,
	task_id: TaskId,
	task_work_table: Arc<TaskWorktable>,
	ack: oneshot::Sender<Result<(), SystemError>>,
) {
	trace!("Task system received a task resume request");
	spawn(
		{
			let workers = Arc::clone(workers);
			async move {
				let (tx, rx) = oneshot::channel();
				let first_attempt_worker_id = task_work_table.worker_id();
				workers[first_attempt_worker_id]
					.resume_task(task_id, tx)
					.await;
				let res = rx
					.await
					.expect("Task system channel closed trying to resume not running task");

				if matches!(res, Err(SystemError::TaskNotFound(_))) {
					warn!(
						%first_attempt_worker_id,
						"Failed the first try to resume a not running task, trying again",
					);
					workers[task_work_table.worker_id()]
						.resume_task(task_id, ack)
						.await;
				} else {
					ack.send(res)
						.expect("System channel closed trying to resume not running task");
				}
			}
		}
		.in_current_span(),
	);
	trace!("Task system resumed task");
}

#[instrument(skip(workers, ack, task_work_table))]
fn dispatch_pause_not_running_task_request<E: RunError>(
	workers: &Arc<Vec<Worker<E>>>,
	task_id: TaskId,
	task_work_table: Arc<TaskWorktable>,
	ack: oneshot::Sender<Result<(), SystemError>>,
) {
	spawn(
		{
			let workers: Arc<Vec<Worker<E>>> = Arc::clone(workers);

			async move {
				let (tx, rx) = oneshot::channel();
				let first_attempt_worker_id = task_work_table.worker_id();
				workers[first_attempt_worker_id]
					.pause_not_running_task(task_id, tx)
					.await;
				let res = rx
					.await
					.expect("Task system channel closed trying to pause not running task");

				if matches!(res, Err(SystemError::TaskNotFound(_))) {
					warn!(
						%first_attempt_worker_id,
						"Failed the first try to pause a not running task, trying again",
					);
					workers[task_work_table.worker_id()]
						.pause_not_running_task(task_id, ack)
						.await;
				} else {
					ack.send(res)
						.expect("System channel closed trying to pause not running task");
				}
			}
		}
		.in_current_span(),
	);
}

#[instrument(skip(workers, ack))]
fn dispatch_cancel_not_running_task_request<E: RunError>(
	workers: &Arc<Vec<Worker<E>>>,
	task_id: TaskId,
	task_work_table: Arc<TaskWorktable>,
	ack: oneshot::Sender<Result<(), SystemError>>,
) {
	trace!("Task system received a task cancel request");
	spawn(
		{
			let workers = Arc::clone(workers);
			async move {
				let (tx, rx) = oneshot::channel();
				let first_attempt_worker_id = task_work_table.worker_id();
				workers[first_attempt_worker_id]
					.cancel_not_running_task(task_id, tx)
					.await;
				let res = rx
					.await
					.expect("Task system channel closed trying to cancel a not running task");

				if matches!(res, Err(SystemError::TaskNotFound(_))) {
					if task_work_table.is_finalized() {
						return ack
							.send(Ok(()))
							.expect("System channel closed trying to cancel a not running task");
					}

					warn!(
						%first_attempt_worker_id,
						"Failed the first try to cancel a not running task, trying again",
					);
					workers[task_work_table.worker_id()]
						.cancel_not_running_task(task_id, ack)
						.await;
				} else {
					ack.send(res)
						.expect("System channel closed trying to cancel not running task");
				}
			}
		}
		.in_current_span(),
	);

	trace!("Task system canceled task");
}

#[instrument(skip(workers, ack))]
fn dispatch_force_abortion_task_request<E: RunError>(
	workers: &Arc<Vec<Worker<E>>>,
	task_id: TaskId,
	task_work_table: Arc<TaskWorktable>,
	ack: oneshot::Sender<Result<(), SystemError>>,
) {
	trace!("Task system received a task force abortion request");
	spawn(
		{
			let workers = Arc::clone(workers);
			async move {
				let (tx, rx) = oneshot::channel();
				let first_attempt_worker_id = task_work_table.worker_id();
				workers[first_attempt_worker_id]
					.force_task_abortion(task_id, tx)
					.await;
				let res = rx.await.expect(
					"Task system channel closed trying to force abortion of a not running task",
				);

				if matches!(res, Err(SystemError::TaskNotFound(_))) {
					warn!(
						%first_attempt_worker_id,
						"Failed the first try to force abortion of a not running task, trying again",
					);
					workers[task_work_table.worker_id()]
						.force_task_abortion(task_id, ack)
						.await;
				} else {
					ack.send(res).expect(
						"System channel closed trying to force abortion of a not running task",
					);
				}
			}
		}
		.in_current_span(),
	);
	trace!("Task system aborted task");
}

/// The default implementation of the task system will create a system with a number of workers equal to the available
/// parallelism in the user's machine.
impl<E: RunError> Default for System<E> {
	fn default() -> Self {
		Self::new()
	}
}

/// SAFETY: Due to usage of refcell we lost `Sync` impl, but we only use it to have a shutdown method
/// receiving `&self` which is called once, and we also use `try_borrow_mut` so we never panic
unsafe impl<E: RunError> Sync for System<E> {}

#[derive(Clone, Debug)]
#[repr(transparent)]
pub struct SystemComm(chan::Sender<SystemMessage>);

impl SystemComm {
	pub fn idle_report(&self, worker_id: usize) {
		let system_tx = self.0.clone();
		spawn(
			async move {
				system_tx
					.send(SystemMessage::IdleReport(worker_id))
					.await
					.expect("System channel closed trying to report idle");
			}
			.in_current_span(),
		);
	}

	pub fn working_report(&self, worker_id: usize) {
		let system_tx = self.0.clone();
		spawn(
			async move {
				system_tx
					.send(SystemMessage::WorkingReport(worker_id))
					.await
					.expect("System channel closed trying to report working");
			}
			.in_current_span(),
		);
	}

	pub fn pause_not_running_task(
		&self,
		task_id: TaskId,
		task_work_table: Arc<TaskWorktable>,
		ack: oneshot::Sender<Result<(), SystemError>>,
	) {
		let system_tx = self.0.clone();
		spawn(
			async move {
				system_tx
					.send(SystemMessage::PauseNotRunningTask {
						task_id,
						task_work_table,
						ack,
					})
					.await
					.expect("System channel closed trying to pause not running task");
			}
			.in_current_span(),
		);
	}

	pub fn cancel_not_running_task(
		&self,
		task_id: TaskId,
		task_work_table: Arc<TaskWorktable>,
		ack: oneshot::Sender<Result<(), SystemError>>,
	) {
		let system_tx = self.0.clone();
		spawn(
			async move {
				system_tx
					.send(SystemMessage::CancelNotRunningTask {
						task_id,
						task_work_table,
						ack,
					})
					.await
					.expect("System channel closed trying to cancel a not running task");
			}
			.in_current_span(),
		);
	}

	pub fn resume_task(
		&self,
		task_id: TaskId,
		task_work_table: Arc<TaskWorktable>,
		ack: oneshot::Sender<Result<(), SystemError>>,
	) {
		let system_tx = self.0.clone();
		spawn(
			async move {
				system_tx
					.send(SystemMessage::ResumeTask {
						task_id,
						task_work_table,
						ack,
					})
					.await
					.expect("System channel closed trying to resume task");
			}
			.in_current_span(),
		);
	}

	pub fn force_abortion(
		&self,
		task_id: TaskId,
		task_work_table: Arc<TaskWorktable>,
		ack: oneshot::Sender<Result<(), SystemError>>,
	) {
		let system_tx = self.0.clone();
		spawn(
			async move {
				system_tx
					.send(SystemMessage::ForceAbortion {
						task_id,
						task_work_table,
						ack,
					})
					.await
					.expect("System channel closed trying to resume task");
			}
			.in_current_span(),
		);
	}
}

/// A remote dispatcher of tasks.
///
/// It can be used to dispatch tasks to the system from other threads or tasks.
/// It uses [`Arc`] internally so it can be cheaply cloned and put inside tasks so tasks can dispatch other tasks.
#[derive(Debug)]
pub struct BaseDispatcher<E: RunError> {
	workers: Arc<Vec<Worker<E>>>,
	idle_workers: Arc<Vec<AtomicBool>>,
	last_worker_id: Arc<AtomicWorkerId>,
	has_shutdown: Arc<AtomicBool>,
}

/// A trait that represents a dispatcher that can be used to dispatch tasks to the system.
/// It can be used to dispatch tasks to the system from other threads or tasks.
///
/// The `E: RunError` error parameter is the error type that the dispatcher can return.
/// Although the [`BaseDispatcher`] which is the default implementation of this trait, will always returns
/// a [`Result`] with the [`TaskHandle`] in the [`Ok`] variant, it can be used to implement a custom
/// fallible dispatcher that returns an [`Err`] variant with a custom error type.
pub trait Dispatcher<E: RunError>: fmt::Debug + Clone + Send + Sync + 'static {
	type DispatchError: RunError;

	/// Dispatches a task to the system, the task will be assigned to a worker and executed as soon as possible.
	fn dispatch(
		&self,
		into_task: impl IntoTask<E>,
	) -> impl Future<Output = Result<TaskHandle<E>, Self::DispatchError>> + Send {
		self.dispatch_boxed(into_task.into_task())
	}

	/// Dispatches an already boxed task to the system, the task will be assigned to a worker and executed as
	/// soon as possible.
	fn dispatch_boxed(
		&self,
		boxed_task: Box<dyn Task<E>>,
	) -> impl Future<Output = Result<TaskHandle<E>, Self::DispatchError>> + Send;

	/// Dispatches many tasks to the system, the tasks will be assigned to workers and executed as soon as possible.
	fn dispatch_many<I: IntoIterator<Item = impl IntoTask<E>> + Send>(
		&self,
		into_tasks: I,
	) -> impl Future<Output = Result<Vec<TaskHandle<E>>, Self::DispatchError>> + Send
	where
		I::IntoIter: Send,
	{
		self.dispatch_many_boxed(into_tasks.into_iter().map(IntoTask::into_task))
	}

	/// Dispatches many already boxed tasks to the system, the tasks will be assigned to workers and executed as
	/// soon as possible.
	fn dispatch_many_boxed(
		&self,
		boxed_tasks: impl IntoIterator<Item = Box<dyn Task<E>>> + Send,
	) -> impl Future<Output = Result<Vec<TaskHandle<E>>, Self::DispatchError>> + Send;
}

impl<E: RunError> Clone for BaseDispatcher<E> {
	fn clone(&self) -> Self {
		Self {
			workers: Arc::clone(&self.workers),
			idle_workers: Arc::clone(&self.idle_workers),
			last_worker_id: Arc::clone(&self.last_worker_id),
			has_shutdown: Arc::clone(&self.has_shutdown),
		}
	}
}

impl<E: RunError> Dispatcher<E> for BaseDispatcher<E> {
	type DispatchError = DispatcherShutdownError<E>;

	#[allow(clippy::missing_panics_doc)]
	async fn dispatch_boxed(
		&self,
		task: Box<dyn Task<E>>,
	) -> Result<TaskHandle<E>, Self::DispatchError> {
		if self.has_shutdown.load(Ordering::Acquire) {
			return Err(DispatcherShutdownError(vec![task]));
		}

		let worker_id = self
				.last_worker_id
				.fetch_update(Ordering::Release, Ordering::Acquire, |last_worker_id| {
					Some((last_worker_id + 1) % self.workers.len())
				})
				.expect("we hardcoded the update function to always return Some(next_worker_id) through dispatcher");

		trace!(%worker_id, task_id = %task.id(), "Dispatching task to worker");

		let handle = self.workers[worker_id].add_task(task).await;

		self.idle_workers[worker_id].store(false, Ordering::Relaxed);

		Ok(handle)
	}

	async fn dispatch_many_boxed(
		&self,
		into_tasks: impl IntoIterator<Item = Box<dyn Task<E>>> + Send,
	) -> Result<Vec<TaskHandle<E>>, Self::DispatchError> {
		if self.has_shutdown.load(Ordering::Acquire) {
			return Err(DispatcherShutdownError(into_tasks.into_iter().collect()));
		}

		let (handles, workers_ids_set) = into_tasks
			.into_iter()
			.zip((0..self.workers.len()).cycle())
			.map(|(task, worker_id)| async move {
				(self.workers[worker_id].add_task(task).await, worker_id)
			})
			.collect::<Vec<_>>()
			.join()
			.await
			.into_iter()
			.unzip::<_, _, Vec<_>, HashSet<_>>();

		for worker_id in workers_ids_set {
			self.idle_workers[worker_id].store(false, Ordering::Relaxed);
		}

		Ok(handles)
	}
}

impl<E: RunError> BaseDispatcher<E> {
	/// Returns the number of workers in the system.
	#[must_use]
	pub fn workers_count(&self) -> usize {
		self.workers.len()
	}
}
