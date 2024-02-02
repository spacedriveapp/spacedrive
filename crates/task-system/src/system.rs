use std::{
	cell::RefCell,
	collections::HashSet,
	pin::pin,
	sync::{atomic::Ordering, Arc},
};

use async_channel as chan;
use futures::StreamExt;
use futures_concurrency::future::Join;
use tokio::{spawn, sync::oneshot, task::JoinHandle};
use tracing::{error, warn};

use super::{
	error::Error,
	message::SystemMessage,
	task::{IntoTask, TaskHandle, TaskId},
	worker::{AtomicWorkerId, WorkStealer, Worker, WorkerBuilder, WorkerId},
};

pub struct System {
	workers: Arc<Vec<Worker>>,
	msgs_tx: chan::Sender<SystemMessage>,
	dispatcher: Dispatcher,
	handle: RefCell<Option<JoinHandle<()>>>,
}

impl System {
	pub async fn new() -> Self {
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

		let workers = Arc::new(
			workers_builders
				.into_iter()
				.map(|builder| builder.build(system_comm.clone(), task_stealer.clone()))
				.collect::<Vec<_>>(),
		);

		let handle = spawn({
			let workers = Arc::clone(&workers);
			let msgs_rx = msgs_rx.clone();

			async move {
				while let Err(e) = spawn(Self::run(Arc::clone(&workers), msgs_rx.clone())).await {
					if e.is_panic() {
						error!("Job system panicked: {e:#?}");
					} else {
						error!("Job system failed: {e:#?}");
					}
				}
			}
		});

		Self {
			workers: Arc::clone(&workers),
			msgs_tx: msgs_tx.clone(),
			dispatcher: Dispatcher {
				workers,
				msgs_tx,
				last_worker_id: Arc::new(AtomicWorkerId::new(0)),
			},

			handle: RefCell::new(Some(handle)),
		}
	}

	pub async fn dispatch(&self, into_task: impl IntoTask) -> TaskHandle {
		self.dispatcher.dispatch(into_task).await
	}

	pub async fn dispatch_many(&self, into_tasks: Vec<impl IntoTask>) -> Vec<TaskHandle> {
		self.dispatcher.dispatch_many(into_tasks).await
	}

	pub fn get_dispatcher(&self) -> Dispatcher {
		self.dispatcher.clone()
	}

	async fn run(workers: Arc<Vec<Worker>>, msgs_rx: chan::Receiver<SystemMessage>) {
		let mut msg_stream = pin!(msgs_rx);

		let mut idle_workers = vec![true; workers.len()];

		while let Some(msg) = msg_stream.next().await {
			match msg {
				SystemMessage::IdleReport(worker_id) => {
					idle_workers[worker_id] = true;
				}

				SystemMessage::WorkingReport(worker_id) => {
					idle_workers[worker_id] = false;
				}

				SystemMessage::ActiveReports(worker_ids) => {
					for worker_id in worker_ids {
						idle_workers[worker_id] = false;
					}
				}

				SystemMessage::ResumeTask {
					task_id,
					worker_id,
					ack,
				} => workers[worker_id].resume_task(task_id, ack).await,

				SystemMessage::ForceAbortion {
					task_id,
					worker_id,
					ack,
				} => workers[worker_id].force_task_abortion(task_id, ack).await,

				SystemMessage::NotifyIdleWorkers {
					start_from,
					task_count,
				} => {
					for idx in (0..workers.len())
						.cycle()
						.skip(start_from)
						.take(usize::min(task_count, workers.len()))
					{
						if idle_workers[idx] {
							workers[idx].wake().await;
							// we don't mark the worker as not idle because we wait for it to
							// successfully steal a task and then report it back as active
						}
					}
				}

				SystemMessage::ShutdownRequest(tx) => {
					tx.send(Ok(()))
						.expect("System channel closed trying to shutdown");
					break;
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

/// SAFETY: Due to usage of refcell we lost `Sync` impl, but we only use it to have a shutdown method
/// receiving `&self` which is called once, and we also use `try_borrow_mut` so we never panic
unsafe impl Sync for System {}

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

#[derive(Clone, Debug)]
pub struct Dispatcher {
	workers: Arc<Vec<Worker>>,
	msgs_tx: chan::Sender<SystemMessage>,
	last_worker_id: Arc<AtomicWorkerId>,
}

impl Dispatcher {
	pub async fn dispatch(&self, into_task: impl IntoTask) -> TaskHandle {
		let task = into_task.into_task();

		#[allow(clippy::async_yields_async)]
		let inner = |task| async {
			let worker_id = self
				.last_worker_id
				.fetch_update(Ordering::Release, Ordering::Acquire, |last_worker_id| {
					Some((last_worker_id + 1) % self.workers.len())
				})
				.expect("we hardcoded the update function to always return Some(next_worker_id) through dispatcher");

			let handle = self.workers[worker_id].add_task(task).await;

			self.msgs_tx
				.send(SystemMessage::ActiveReports(vec![worker_id]))
				.await
				.expect("System channel closed trying to report active worker through dispatcher");

			handle
		};

		inner(task).await
	}

	pub async fn dispatch_many(&self, into_tasks: Vec<impl IntoTask>) -> Vec<TaskHandle> {
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

		self.msgs_tx
			.send(SystemMessage::ActiveReports(
				workers_ids_set.into_iter().collect(),
			))
			.await
			.expect("System channel closed trying to report active workers");

		handles
	}
}
