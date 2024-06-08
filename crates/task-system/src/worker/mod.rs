use std::{
	cell::RefCell,
	sync::{atomic::AtomicUsize, Arc},
	time::Duration,
};

use async_channel as chan;
use tokio::{spawn, sync::oneshot, task::JoinHandle};
use tracing::{error, info, instrument, trace, warn, Instrument};

use super::{
	error::{RunError, SystemError},
	message::{StoleTaskMessage, TaskRunnerOutput, WorkerMessage},
	system::SystemComm,
	task::{
		Interrupter, PanicOnSenderDrop, Task, TaskHandle, TaskId, TaskRemoteController,
		TaskWorkState, TaskWorktable,
	},
};

mod run;
mod runner;

use run::run;

const ONE_SECOND: Duration = Duration::from_secs(1);

pub type WorkerId = usize;
pub type AtomicWorkerId = AtomicUsize;

pub struct WorkerBuilder<E: RunError> {
	id: usize,
	msgs_tx: chan::Sender<WorkerMessage<E>>,
	msgs_rx: chan::Receiver<WorkerMessage<E>>,
}

impl<E: RunError> WorkerBuilder<E> {
	pub fn new(id: WorkerId) -> (Self, WorkerComm<E>) {
		let (msgs_tx, msgs_rx) = chan::bounded(8);

		let worker_comm = WorkerComm {
			worker_id: id,
			msgs_tx: msgs_tx.clone(),
		};

		(
			Self {
				id,
				msgs_tx,
				msgs_rx,
			},
			worker_comm,
		)
	}

	#[instrument(name = "task_system_worker", skip(self, system_comm, task_stealer), fields(worker_id = self.id))]
	pub fn build(self, system_comm: SystemComm, task_stealer: WorkStealer<E>) -> Worker<E> {
		let Self {
			id,
			msgs_tx,
			msgs_rx,
		} = self;

		let handle = spawn({
			let system_comm = system_comm.clone();

			async move {
				trace!("Worker message processing task starting...");
				while let Err(e) = spawn(run(
					id,
					system_comm.clone(),
					task_stealer.clone(),
					msgs_rx.clone(),
				))
				.await
				{
					if e.is_panic() {
						error!(?e, "Worker critically failed and will restart;");
					} else {
						trace!("Worker received shutdown signal and will exit...");
						break;
					}
				}

				info!("Worker gracefully shutdown");
			}
			.in_current_span()
		});

		Worker {
			id,
			system_comm,
			msgs_tx,
			handle: RefCell::new(Some(handle)),
		}
	}
}

#[derive(Debug)]
pub struct Worker<E: RunError> {
	pub id: usize,
	system_comm: SystemComm,
	msgs_tx: chan::Sender<WorkerMessage<E>>,
	handle: RefCell<Option<JoinHandle<()>>>,
}

impl<E: RunError> Worker<E> {
	pub async fn add_task(&self, new_task: Box<dyn Task<E>>) -> TaskHandle<E> {
		let (done_tx, done_rx) = oneshot::channel();

		let (interrupt_tx, interrupt_rx) = chan::bounded(1);

		let worktable = Arc::new(TaskWorktable::new(self.id, interrupt_tx));

		let task_id = new_task.id();

		self.msgs_tx
			.send(WorkerMessage::NewTask(TaskWorkState {
				task: new_task,
				worktable: Arc::clone(&worktable),
				interrupter: Arc::new(Interrupter::new(interrupt_rx)),
				done_tx: PanicOnSenderDrop::new(task_id, done_tx),
			}))
			.await
			.expect("Worker channel closed trying to add task");

		TaskHandle {
			done_rx,
			controller: TaskRemoteController {
				worktable,
				system_comm: self.system_comm.clone(),
				task_id,
			},
		}
	}

	pub async fn resume_task(
		&self,
		task_id: TaskId,
		ack: oneshot::Sender<Result<(), SystemError>>,
	) {
		self.msgs_tx
			.send(WorkerMessage::ResumeTask { task_id, ack })
			.await
			.expect("Worker channel closed trying to resume task");
	}

	pub async fn pause_not_running_task(
		&self,
		task_id: TaskId,
		ack: oneshot::Sender<Result<(), SystemError>>,
	) {
		self.msgs_tx
			.send(WorkerMessage::PauseNotRunningTask { task_id, ack })
			.await
			.expect("Worker channel closed trying to pause a not running task");
	}

	pub async fn cancel_not_running_task(
		&self,
		task_id: TaskId,
		ack: oneshot::Sender<Result<(), SystemError>>,
	) {
		self.msgs_tx
			.send(WorkerMessage::CancelNotRunningTask { task_id, ack })
			.await
			.expect("Worker channel closed trying to cancel a not running task");
	}

	pub async fn force_task_abortion(
		&self,
		task_id: TaskId,
		ack: oneshot::Sender<Result<(), SystemError>>,
	) {
		self.msgs_tx
			.send(WorkerMessage::ForceAbortion { task_id, ack })
			.await
			.expect("Worker channel closed trying to force task abortion");
	}

	#[instrument(skip(self), fields(worker_id = self.id))]
	pub async fn shutdown(&self) {
		if let Some(handle) = self
			.handle
			.try_borrow_mut()
			.ok()
			.and_then(|mut maybe_handle| maybe_handle.take())
		{
			let (tx, rx) = oneshot::channel();

			self.msgs_tx
				.send(WorkerMessage::ShutdownRequest(tx))
				.await
				.expect("Worker channel closed trying to shutdown");

			rx.await.expect("Worker channel closed trying to shutdown");

			if let Err(e) = handle.await {
				if e.is_panic() {
					error!("Worker {} critically failed: {e:#?}", self.id);
				}
			}
		} else {
			warn!("Trying to shutdown a worker that was already shutdown");
		}
	}
}

/// SAFETY: Due to usage of refcell we lost `Sync` impl, but we only use it to have a shutdown method
/// receiving `&self` which is called once, and we also use `try_borrow_mut` so we never panic
unsafe impl<E: RunError> Sync for Worker<E> {}

#[derive(Clone)]
pub struct WorkerComm<E: RunError> {
	worker_id: WorkerId,
	msgs_tx: chan::Sender<WorkerMessage<E>>,
}

impl<E: RunError> WorkerComm<E> {
	pub async fn steal_task(
		&self,
		stealer_id: WorkerId,
		stolen_task_tx: chan::Sender<Option<StoleTaskMessage<E>>>,
	) -> bool {
		let (tx, rx) = oneshot::channel();

		self.msgs_tx
			.send(WorkerMessage::StealRequest {
				stealer_id,
				ack: tx,
				stolen_task_tx,
			})
			.await
			.expect("Worker channel closed trying to steal task");

		rx.await
			.expect("Worker channel closed trying to steal task")
	}
}

pub struct WorkStealer<E: RunError> {
	worker_comms: Arc<Vec<WorkerComm<E>>>,
}

impl<E: RunError> Clone for WorkStealer<E> {
	fn clone(&self) -> Self {
		Self {
			worker_comms: Arc::clone(&self.worker_comms),
		}
	}
}

impl<E: RunError> WorkStealer<E> {
	pub fn new(worker_comms: Vec<WorkerComm<E>>) -> Self {
		Self {
			worker_comms: Arc::new(worker_comms),
		}
	}

	#[instrument(skip(self, stolen_task_tx))]
	pub async fn steal(
		&self,
		stealer_id: WorkerId,
		stolen_task_tx: &chan::Sender<Option<StoleTaskMessage<E>>>,
	) {
		let total_workers = self.worker_comms.len();

		for worker_comm in self
			.worker_comms
			.iter()
			// Cycling over the workers
			.cycle()
			// Starting from the next worker id
			.skip(stealer_id)
			// Taking the total amount of workers
			.take(total_workers)
			// Removing the current worker as we can't steal from ourselves
			.filter(|worker_comm| worker_comm.worker_id != stealer_id)
		{
			if worker_comm
				.steal_task(stealer_id, stolen_task_tx.clone())
				.await
			{
				trace!(stolen_worker_id = worker_comm.worker_id, "Stole a task");
				return;
			}
		}

		stolen_task_tx
			.send(None)
			.await
			.expect("Stolen task channel closed");
	}
}
