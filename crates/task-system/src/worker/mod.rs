use std::{
	cell::RefCell,
	sync::{atomic::AtomicUsize, Arc},
};

use async_channel as chan;
use tokio::{spawn, sync::oneshot, task::JoinHandle};
use tracing::{error, info, warn};

use crate::task::Interrupter;

use super::{
	error::Error,
	message::WorkerMessage,
	system::SystemComm,
	task::{DynTask, TaskHandle, TaskId, TaskWorkState, TaskWorktable},
};

mod run;

use run::run;

pub type WorkerId = usize;
pub(crate) type AtomicWorkerId = AtomicUsize;

pub(crate) struct WorkerBuilder {
	id: usize,
	msgs_tx: chan::Sender<WorkerMessage>,
	msgs_rx: chan::Receiver<WorkerMessage>,
}

impl WorkerBuilder {
	pub fn new(id: WorkerId) -> (Self, WorkerComm) {
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

	pub fn build(self, system_comm: SystemComm, task_stealer: WorkStealer) -> Worker {
		let Self {
			id,
			msgs_tx,
			msgs_rx,
		} = self;

		let handle = spawn({
			let msgs_rx = msgs_rx.clone();
			let system_comm = system_comm.clone();
			let task_stealer = task_stealer.clone();

			async move {
				loop {
					if let Err(e) = spawn(run(
						id,
						system_comm.clone(),
						task_stealer.clone(),
						msgs_rx.clone(),
					))
					.await
					{
						if e.is_panic() {
							error!("Worker {id} critically failed and will restart: {e:#?}");
							continue;
						}
					}

					info!("Worker {id} gracefully shutdown");
					break;
				}
			}
		});

		Worker {
			id,
			system_comm,
			msgs_tx,
			handle: RefCell::new(Some(handle)),
		}
	}
}

pub(crate) struct Worker {
	pub id: usize,
	system_comm: SystemComm,
	msgs_tx: chan::Sender<WorkerMessage>,
	handle: RefCell<Option<JoinHandle<()>>>,
}

impl Worker {
	pub async fn add_task(&self, new_task: DynTask) -> TaskHandle {
		let (done_tx, done_rx) = oneshot::channel();

		let (interrupt_tx, interrupt_rx) = chan::bounded(1);

		let worktable = Arc::new(TaskWorktable::new(self.id, interrupt_tx));

		let task_id = new_task.id();

		self.msgs_tx
			.send(WorkerMessage::NewTask(TaskWorkState {
				task: new_task,
				worktable: Arc::clone(&worktable),
				interrupter: Arc::new(Interrupter::new(interrupt_rx)),
				done_tx,
			}))
			.await
			.expect("Worker channel closed trying to add task");

		TaskHandle {
			worktable,
			done_rx,
			system_comm: self.system_comm.clone(),
			task_id,
		}
	}

	pub async fn task_count(&self) -> usize {
		let (tx, rx) = oneshot::channel();

		self.msgs_tx
			.send(WorkerMessage::TaskCountRequest(tx))
			.await
			.expect("Worker channel closed trying to get task count");

		rx.await
			.expect("Worker channel closed trying to receive task count response")
	}

	pub async fn resume_task(&self, task_id: TaskId, ack: oneshot::Sender<Result<(), Error>>) {
		self.msgs_tx
			.send(WorkerMessage::ResumeTask { task_id, ack })
			.await
			.expect("Worker channel closed trying to resume task");
	}

	pub async fn force_task_abortion(
		&self,
		task_id: TaskId,
		ack: oneshot::Sender<Result<(), Error>>,
	) {
		self.msgs_tx
			.send(WorkerMessage::ForceAbortion { task_id, ack })
			.await
			.expect("Worker channel closed trying to force task abortion");
	}

	pub async fn shutdown(&self) -> Vec<DynTask> {
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

			let tasks = rx.await.expect("Worker channel closed trying to shutdown");

			if let Err(e) = handle.await {
				if e.is_panic() {
					error!("Worker {} critically failed: {e:#?}", self.id);
				}
			}

			tasks
		} else {
			warn!("Trying to shutdown a worker that was already shutdown");
			Vec::new()
		}
	}

	pub async fn wake(&self) {
		self.msgs_tx
			.send(WorkerMessage::WakeUp)
			.await
			.expect("Worker channel closed trying to wake up");
	}
}

/// SAFETY: Due to usage of refcell we lost `Sync` impl, but we only use it to have a shutdown method
/// receiving `&self` which is called once, and we also use `try_borrow_mut` so we never panic
unsafe impl Sync for Worker {}

#[derive(Clone)]
pub(crate) struct WorkerComm {
	worker_id: WorkerId,
	msgs_tx: chan::Sender<WorkerMessage>,
}

impl WorkerComm {
	pub async fn steal_task(&self, worker_id: WorkerId) -> Option<TaskWorkState> {
		let (tx, rx) = oneshot::channel();

		self.msgs_tx
			.send(WorkerMessage::StealRequest(tx))
			.await
			.expect("Worker channel closed trying to steal task");

		rx.await
			.expect("Worker channel closed trying to steal task")
			.map(|task_work_state| {
				task_work_state.change_worker(worker_id);
				task_work_state
			})
	}
}

#[derive(Clone)]
pub(crate) struct WorkStealer {
	worker_comms: Arc<Vec<WorkerComm>>,
}

impl WorkStealer {
	pub fn new(worker_comms: Vec<WorkerComm>) -> Self {
		Self {
			worker_comms: Arc::new(worker_comms),
		}
	}

	pub async fn steal(&self, worker_id: WorkerId) -> Option<TaskWorkState> {
		let total_workers = self.worker_comms.len();

		for worker_comm in self
			.worker_comms
			.iter()
			// Cycling over the workers
			.cycle()
			// Starting from the next worker id
			.skip(worker_id)
			// Taking the total amount of workers
			.take(total_workers)
			// Removing the current worker as we can't steal from ourselves
			.filter(|worker_comm| worker_comm.worker_id != worker_id)
		{
			if let Some(task) = worker_comm.steal_task(worker_id).await {
				return Some(task);
			}
		}

		None
	}

	pub fn workers_count(&self) -> usize {
		self.worker_comms.len()
	}
}
