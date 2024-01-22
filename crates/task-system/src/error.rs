use sd_utils::error::FileIOError;

use rmp_serde::{decode, encode};
use thiserror::Error;

use super::task::TaskId;

#[derive(Debug, Error)]
pub enum Error {
	#[error("missing loader for task kind: {0}")]
	MissingLoader(&'static str),
	#[error("task not found: {0}")]
	TaskNotFound(TaskId),
	#[error("task aborted <id='{0}'>")]
	TaskAborted(TaskId),
	#[error("task join error <id='{0}'>")]
	TaskJoin(TaskId),
	#[error("forced abortion for task <id='{0}'> timed out")]
	TaskForcedAbortTimeout(TaskId),

	#[error("serialize error: {0}")]
	Serialize(#[from] encode::Error),
	#[error("deserialize error: {0}")]
	Deserialize(#[from] decode::Error),

	#[error(transparent)]
	FileIO(#[from] FileIOError),

	#[error("task store file not found")]
	TaskStoreFileNotFound,
}
