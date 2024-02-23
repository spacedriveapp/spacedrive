use sd_utils::error::FileIOError;

use thiserror::Error;

use super::task::TaskId;

#[derive(Debug, Error)]
pub enum Error {
	#[error("task not found: {0}")]
	TaskNotFound(TaskId),
	#[error("task aborted <id='{0}'>")]
	TaskAborted(TaskId),
	#[error("task join error <id='{0}'>")]
	TaskJoin(TaskId),
	#[error("forced abortion for task <id='{0}'> timed out")]
	TaskForcedAbortTimeout(TaskId),

	#[error(transparent)]
	FileIO(#[from] FileIOError),
}
