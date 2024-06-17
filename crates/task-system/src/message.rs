use std::sync::Arc;

use async_channel as chan;
use tokio::sync::oneshot;

use super::{
	error::{RunError, SystemError},
	task::{InternalTaskExecStatus, TaskId, TaskWorkState, TaskWorktable},
	worker::WorkerId,
};

#[derive(Debug)]
pub enum SystemMessage {
	IdleReport(WorkerId),
	WorkingReport(WorkerId),
	ResumeTask {
		task_id: TaskId,
		task_work_table: Arc<TaskWorktable>,
		ack: oneshot::Sender<Result<(), SystemError>>,
	},
	PauseNotRunningTask {
		task_id: TaskId,
		task_work_table: Arc<TaskWorktable>,
		ack: oneshot::Sender<Result<(), SystemError>>,
	},
	CancelNotRunningTask {
		task_id: TaskId,
		task_work_table: Arc<TaskWorktable>,
		ack: oneshot::Sender<Result<(), SystemError>>,
	},
	ForceAbortion {
		task_id: TaskId,
		task_work_table: Arc<TaskWorktable>,
		ack: oneshot::Sender<Result<(), SystemError>>,
	},
	ShutdownRequest(oneshot::Sender<Result<(), SystemError>>),
}

pub enum WorkerMessage<E: RunError> {
	NewTask(TaskWorkState<E>),
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
		ack: oneshot::Sender<Result<(), SystemError>>,
	},
	ForceAbortion {
		task_id: TaskId,
		ack: oneshot::Sender<Result<(), SystemError>>,
	},
	ShutdownRequest(oneshot::Sender<()>),
	StealRequest {
		stealer_id: WorkerId,
		ack: oneshot::Sender<bool>,
		stolen_task_tx: chan::Sender<Option<StoleTaskMessage<E>>>,
	},
}

pub struct TaskRunnerOutput<E: RunError> {
	pub task_work_state: TaskWorkState<E>,
	pub status: InternalTaskExecStatus<E>,
}

pub struct TaskOutputMessage<E: RunError>(pub TaskId, pub Result<TaskRunnerOutput<E>, ()>);

pub struct StoleTaskMessage<E: RunError>(pub TaskWorkState<E>);
