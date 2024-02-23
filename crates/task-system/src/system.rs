use std::{
	cell::RefCell,
	collections::HashSet,
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
use tracing::{error, info, trace, warn};

use super::{
	error::Error,
	message::SystemMessage,
	task::{IntoTask, Task, TaskHandle, TaskId, TaskRunError},
	worker::{AtomicWorkerId, WorkStealer, Worker, WorkerBuilder, WorkerId},
};

pub struct System<E: TaskRunError> {
	workers: Arc<Vec<Worker<E>>>,
	msgs_tx: chan::Sender<SystemMessage>,
	dispatcher: Dispatcher<E>,
	handle: RefCell<Option<JoinHandle<()>>>,
}

impl<E: TaskRunError> System<E> {
	pub fn new() -> Self {
		let workers_count = std::thread::available_parallelism().map_or_else(
			|e| {
				error!("Failed to get available parallelism in the job system: {e:#?}");
				1
			},
			|non_zero| non_zero.get(),
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
			let msgs_rx = msgs_rx.clone();
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
						error!("Job system panicked: {e:#?}");
					} else {
						trace!("Task system received shutdown signal and will exit...");
						break;
					}
					trace!("Restarting task system message processing task...")
				}

				info!("Task system gracefully shutdown");
			}
		});

		trace!("Task system online!");

		Self {
			workers: Arc::clone(&workers),
			msgs_tx,
			dispatcher: Dispatcher {
				workers,
				idle_workers,
				last_worker_id: Arc::new(AtomicWorkerId::new(0)),
			},

			handle: RefCell::new(Some(handle)),
		}
	}

	pub fn workers_count(&self) -> usize {
		self.workers.len()
	}

	pub async fn dispatch(&self, into_task: impl IntoTask<E>) -> TaskHandle<E> {
		self.dispatcher.dispatch(into_task).await
	}

	pub async fn dispatch_many(&self, into_tasks: Vec<impl IntoTask<E>>) -> Vec<TaskHandle<E>> {
		self.dispatcher.dispatch_many(into_tasks).await
	}

	pub fn get_dispatcher(&self) -> Dispatcher<E> {
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
					trace!("Task system received a worker idle report request: <worker_id='{worker_id}'>");
					idle_workers[worker_id].store(true, Ordering::Relaxed);
				}

				SystemMessage::WorkingReport(worker_id) => {
					trace!(
						"Task system received a working report request: <worker_id='{worker_id}'>"
					);
					idle_workers[worker_id].store(false, Ordering::Relaxed);
				}

				SystemMessage::ResumeTask {
					task_id,
					worker_id,
					ack,
				} => {
					trace!("Task system received a task resume request: <task_id='{task_id}', worker_id='{worker_id}'>");
					workers[worker_id].resume_task(task_id, ack).await;
				}

				SystemMessage::PauseNotRunningTask {
					task_id,
					worker_id,
					ack,
				} => {
					trace!("Task system received a task resume request: <task_id='{task_id}', worker_id='{worker_id}'>");
					workers[worker_id]
						.pause_not_running_task(task_id, ack)
						.await;
				}

				SystemMessage::CancelNotRunningTask {
					task_id,
					worker_id,
					ack,
				} => {
					trace!("Task system received a task resume request: <task_id='{task_id}', worker_id='{worker_id}'>");
					workers[worker_id]
						.cancel_not_running_task(task_id, ack)
						.await;
				}

				SystemMessage::ForceAbortion {
					task_id,
					worker_id,
					ack,
				} => {
					trace!(
						"Task system received a task force abortion request: \
						<task_id='{task_id}', worker_id='{worker_id}'>"
					);
					workers[worker_id].force_task_abortion(task_id, ack).await;
				}

				SystemMessage::NotifyIdleWorkers {
					start_from,
					task_count,
				} => {
					trace!(
						"Task system received a request to notify idle workers: \
						<start_from='{start_from}', task_count='{task_count}'>"
					);

					for idx in (0..workers.len())
						.cycle()
						.skip(start_from)
						.take(usize::min(task_count, workers.len()))
					{
						if idle_workers[idx].load(Ordering::Relaxed) {
							workers[idx].wake().await;
							// we don't mark the worker as not idle because we wait for it to
							// successfully steal a task and then report it back as active
						}
					}
				}

				SystemMessage::ShutdownRequest(tx) => {
					trace!("Task system received a shutdown request");
					tx.send(Ok(()))
						.expect("System channel closed trying to shutdown");
					return;
				}
			}
		}
	}

	pub async fn shutdown(&self) {
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
				error!("Task system failed to shutdown on handle await: {e:#?}");
			}
		} else {
			warn!("Trying to shutdown the tasks system that was already shutdown");
		}
	}
}

impl<E: TaskRunError> Default for System<E> {
	fn default() -> Self {
		Self::new()
	}
}

/// SAFETY: Due to usage of refcell we lost `Sync` impl, but we only use it to have a shutdown method
/// receiving `&self` which is called once, and we also use `try_borrow_mut` so we never panic
unsafe impl<E: TaskRunError> Sync for System<E> {}

#[derive(Clone, Debug)]
#[repr(transparent)]
pub(crate) struct SystemComm(chan::Sender<SystemMessage>);

impl SystemComm {
	pub async fn idle_report(&self, worker_id: usize) {
		self.0
			.send(SystemMessage::IdleReport(worker_id))
			.await
			.expect("System channel closed trying to report idle");
	}

	pub async fn working_report(&self, worker_id: usize) {
		self.0
			.send(SystemMessage::WorkingReport(worker_id))
			.await
			.expect("System channel closed trying to report working");
	}

	pub async fn pause_not_running_task(
		&self,
		task_id: TaskId,
		worker_id: WorkerId,
	) -> Result<(), Error> {
		let (tx, rx) = oneshot::channel();

		self.0
			.send(SystemMessage::PauseNotRunningTask {
				task_id,
				worker_id,
				ack: tx,
			})
			.await
			.expect("System channel closed trying to pause not running task");

		rx.await
			.expect("System channel closed trying receive pause not running task response")
	}

	pub async fn cancel_not_running_task(
		&self,
		task_id: TaskId,
		worker_id: WorkerId,
	) -> Result<(), Error> {
		let (tx, rx) = oneshot::channel();

		self.0
			.send(SystemMessage::CancelNotRunningTask {
				task_id,
				worker_id,
				ack: tx,
			})
			.await
			.expect("System channel closed trying to cancel a not running task");

		rx.await
			.expect("System channel closed trying receive cancel a not running task response")
	}

	pub async fn request_help(&self, worker_id: WorkerId, task_count: usize) {
		self.0
			.send(SystemMessage::NotifyIdleWorkers {
				start_from: worker_id,
				task_count,
			})
			.await
			.expect("System channel closed trying to request help");
	}

	pub async fn resume_task(&self, task_id: TaskId, worker_id: WorkerId) -> Result<(), Error> {
		let (tx, rx) = oneshot::channel();

		self.0
			.send(SystemMessage::ResumeTask {
				task_id,
				worker_id,
				ack: tx,
			})
			.await
			.expect("System channel closed trying to resume task");

		rx.await
			.expect("System channel closed trying receive resume task response")
	}

	pub async fn force_abortion(&self, task_id: TaskId, worker_id: WorkerId) -> Result<(), Error> {
		let (tx, rx) = oneshot::channel();

		self.0
			.send(SystemMessage::ForceAbortion {
				task_id,
				worker_id,
				ack: tx,
			})
			.await
			.expect("System channel closed trying to resume task");

		rx.await
			.expect("System channel closed trying receive resume task response")
	}
}

#[derive(Debug)]
pub struct Dispatcher<E: TaskRunError> {
	workers: Arc<Vec<Worker<E>>>,
	idle_workers: Arc<Vec<AtomicBool>>,
	last_worker_id: Arc<AtomicWorkerId>,
}

impl<E: TaskRunError> Clone for Dispatcher<E> {
	fn clone(&self) -> Self {
		Self {
			workers: Arc::clone(&self.workers),
			idle_workers: Arc::clone(&self.idle_workers),
			last_worker_id: Arc::clone(&self.last_worker_id),
		}
	}
}

impl<E: TaskRunError> Dispatcher<E> {
	pub async fn dispatch(&self, into_task: impl IntoTask<E>) -> TaskHandle<E> {
		let task = into_task.into_task();

		async fn inner<E: TaskRunError>(
			this: &Dispatcher<E>,
			task: Box<dyn Task<E>>,
		) -> TaskHandle<E> {
			let worker_id = this
				.last_worker_id
				.fetch_update(Ordering::Release, Ordering::Acquire, |last_worker_id| {
					Some((last_worker_id + 1) % this.workers.len())
				})
				.expect("we hardcoded the update function to always return Some(next_worker_id) through dispatcher");

			trace!(
				"Dispatching task to worker: <worker_id='{worker_id}', task_id='{}'>",
				task.id()
			);
			let handle = this.workers[worker_id].add_task(task).await;

			this.idle_workers[worker_id].store(false, Ordering::Relaxed);

			handle
		}

		inner(self, task).await
	}

	pub async fn dispatch_many(&self, into_tasks: Vec<impl IntoTask<E>>) -> Vec<TaskHandle<E>> {
		let mut workers_task_count = self
			.workers
			.iter()
			.map(|worker| async move { (worker.id, worker.task_count().await) })
			.collect::<Vec<_>>()
			.join()
			.await;

		workers_task_count.sort_by_key(|(_id, count)| *count);

		let (handles, workers_ids_set) = into_tasks
			.into_iter()
			.map(IntoTask::into_task)
			.zip(workers_task_count.into_iter().cycle())
			.map(|(task, (worker_id, _))| async move {
				(self.workers[worker_id].add_task(task).await, worker_id)
			})
			.collect::<Vec<_>>()
			.join()
			.await
			.into_iter()
			.unzip::<_, _, Vec<_>, HashSet<_>>();

		workers_ids_set.into_iter().for_each(|worker_id| {
			self.idle_workers[worker_id].store(false, Ordering::Relaxed);
		});

		handles
	}

	pub fn workers_count(&self) -> usize {
		self.workers.len()
	}
}
