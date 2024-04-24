use tokio::sync::oneshot;

use super::{
	error::{RunError, SystemError},
	task::{TaskId, TaskWorkState},
	worker::WorkerId,
};

#[derive(Debug)]
pub enum SystemMessage {
	IdleReport(WorkerId),
	WorkingReport(WorkerId),
	ResumeTask {
		task_id: TaskId,
		worker_id: WorkerId,
		ack: oneshot::Sender<Result<(), SystemError>>,
	},
	PauseNotRunningTask {
		task_id: TaskId,
		worker_id: WorkerId,
		ack: oneshot::Sender<Result<(), SystemError>>,
	},
	CancelNotRunningTask {
		task_id: TaskId,
		worker_id: WorkerId,
		ack: oneshot::Sender<()>,
	},
	ForceAbortion {
		task_id: TaskId,
		worker_id: WorkerId,
		ack: oneshot::Sender<Result<(), SystemError>>,
	},
	NotifyIdleWorkers {
		start_from: WorkerId,
		task_count: usize,
	},
	ShutdownRequest(oneshot::Sender<Result<(), SystemError>>),
}

#[derive(Debug)]
pub enum WorkerMessage<E: RunError> {
	NewTask(TaskWorkState<E>),
	TaskCountRequest(oneshot::Sender<usize>),
	ResumeTask {
		task_id: TaskId,
		ack: oneshot::Sender<Result<(), SystemError>>,
	},
	PauseNotRunningTask {
		task_id: TaskId,
		ack: oneshot::Sender<Result<(), SystemError>>,
	},
	CancelNotRunningTask {
		task_id: TaskId,
		ack: oneshot::Sender<()>,
	},
	ForceAbortion {
		task_id: TaskId,
		ack: oneshot::Sender<Result<(), SystemError>>,
	},
	ShutdownRequest(oneshot::Sender<()>),
	StealRequest(oneshot::Sender<Option<TaskWorkState<E>>>),
	WakeUp,
}
