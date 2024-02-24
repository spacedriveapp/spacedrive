//!
//! # Task System
//!
//! Spacedrive's Task System is a library that provides a way to manage and execute tasks in a concurrent
//! and parallel environment.
//!
//! Just bring your own unified error type and dispatch some tasks, the system will handle enqueueing,
//! parallel execution, and error handling for you. Aside from some niceties like:
//! - Round robin scheduling between workers following the available CPU cores on the user machine;
//! - Work stealing between workers for better load balancing;
//! - Gracefully pause and cancel tasks;
//! - Forced abortion of tasks;
//! - Prioritizing tasks that will suspend running tasks without priority;
//! - When the system is shutdown, it will return all pending and running tasks to theirs dispatchers, so the user can store them on disk or any other storage to be re-dispatched later;
//!
//!
//! ## Basic example
//!
//! ```
//! use sd_task_system::{TaskSystem, Task, TaskId, ExecStatus, TaskOutput, Interrupter, TaskStatus};
//! use async_trait::async_trait;
//! use thiserror::Error;
//!
//! #[derive(Debug, Error)]
//! pub enum SampleError {
//!     #[error("Sample error")]
//!     SampleError,
//! }
//!
//! #[derive(Debug)]
//! pub struct ReadyTask {
//!     id: TaskId,
//! }
//!
//! #[async_trait]
//! impl Task<SampleError> for ReadyTask {
//!     fn id(&self) -> TaskId {
//!         self.id
//!     }
//!
//!     async fn run(&mut self, _interrupter: &Interrupter) -> Result<ExecStatus, SampleError> {
//!         Ok(ExecStatus::Done(TaskOutput::Empty))
//!     }
//! }
//!
//! #[tokio::main]
//! async fn main() {
//!     let system = TaskSystem::new();
//!
//!     let handle = system.dispatch(ReadyTask { id: TaskId::new_v4() }).await;
//!
//!     assert!(matches!(
//!         handle.await,
//!         Ok(TaskStatus::Done(TaskOutput::Empty))
//!     ));
//!
//!     system.shutdown().await;
//! }
//! ```
mod error;
mod message;
mod system;
mod task;
mod worker;

pub use error::{RunError, SystemError as TaskSystemError};
pub use system::{Dispatcher as TaskDispatcher, System as TaskSystem};
pub use task::{
	AnyTaskOutput, ExecStatus, Interrupter, InterrupterFuture, InterruptionKind, IntoAnyTaskOutput,
	IntoTask, Task, TaskHandle, TaskId, TaskOutput, TaskStatus,
};
