use std::{error::Error, fmt};

use super::task::{Task, TaskId};

/// Task system's error type definition, representing when internal errors occurs.
#[derive(Debug, thiserror::Error)]
pub enum SystemError {
	#[error("task not found <task_id='{0}'>")]
	TaskNotFound(TaskId),
	#[error("task aborted <task_id='{0}'>")]
	TaskAborted(TaskId),
	#[error("task join error <task_id='{0}'>")]
	TaskJoin(TaskId),
	#[error("task timeout error <task_id='{0}'>")]
	TaskTimeout(TaskId),
	#[error("forced abortion for task <task_id='{0}'> timed out")]
	TaskForcedAbortTimeout(TaskId),
}

/// Trait for errors that can be returned by tasks, we use this trait as a bound for the task system generic
/// error type.
///
///With this trait, we can have a unified error type through all the tasks in the system.
pub trait RunError: Error + fmt::Debug + Send + Sync + 'static {}

/// We provide a blanket implementation for all types that also implements
/// [`std::error::Error`](https://doc.rust-lang.org/std/error/trait.Error.html) and
/// [`std::fmt::Debug`](https://doc.rust-lang.org/std/fmt/trait.Debug.html).
/// So you will not need to implement this trait for your error type, just implement the `Error` and `Debug`
impl<T: Error + fmt::Debug + Send + Sync + 'static> RunError for T {}

/// A task system dispatcher error type, returning tasks when the task system has shutdown.
#[derive(Debug, thiserror::Error)]
#[error("task system already shutdown and can't dispatch more tasks: <tasks_count={}>", .0.len())]
pub struct DispatcherShutdownError<E: RunError>(pub Vec<Box<dyn Task<E>>>);
