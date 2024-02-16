use tokio::sync::oneshot;

use super::{
	error::Error,
	task::{TaskId, TaskRunError, TaskWorkState},
	worker::WorkerId,
};

#[derive(Debug)]
pub(crate) enum SystemMessage {
	IdleReport(WorkerId),
	WorkingReport(WorkerId),
	ResumeTask {
		task_id: TaskId,
		worker_id: WorkerId,
		ack: oneshot::Sender<Result<(), Error>>,
	},
	PauseNotRunningTask {
		task_id: TaskId,
		worker_id: WorkerId,
		ack: oneshot::Sender<Result<(), Error>>,
	},
	CancelNotRunningTask {
		task_id: TaskId,
		worker_id: WorkerId,
		ack: oneshot::Sender<Result<(), Error>>,
	},
	ForceAbortion {
		task_id: TaskId,
		worker_id: WorkerId,
		ack: oneshot::Sender<Result<(), Error>>,
	},
	NotifyIdleWorkers {
		start_from: WorkerId,
		task_count: usize,
	},
	ShutdownRequest(oneshot::Sender<Result<(), Error>>),
}

#[derive(Debug)]
pub(crate) enum WorkerMessage<E: TaskRunError> {
	NewTask(TaskWorkState<E>),
	TaskCountRequest(oneshot::Sender<usize>),
	ResumeTask {
		task_id: TaskId,
		ack: oneshot::Sender<Result<(), Error>>,
	},
	PauseNotRunningTask {
		task_id: TaskId,
		ack: oneshot::Sender<Result<(), Error>>,
	},
	CancelNotRunningTask {
		task_id: TaskId,
		ack: oneshot::Sender<Result<(), Error>>,
	},
	ForceAbortion {
		task_id: TaskId,
		ack: oneshot::Sender<Result<(), Error>>,
	},
	ShutdownRequest(oneshot::Sender<()>),
	StealRequest(oneshot::Sender<Option<TaskWorkState<E>>>),
	WakeUp,
}
