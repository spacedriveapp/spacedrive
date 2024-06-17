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

#![warn(
	clippy::all,
	clippy::pedantic,
	clippy::correctness,
	clippy::perf,
	clippy::style,
	clippy::suspicious,
	clippy::complexity,
	clippy::nursery,
	clippy::unwrap_used,
	unused_qualifications,
	rust_2018_idioms,
	trivial_casts,
	trivial_numeric_casts,
	unused_allocation,
	clippy::unnecessary_cast,
	clippy::cast_lossless,
	clippy::cast_possible_truncation,
	clippy::cast_possible_wrap,
	clippy::cast_precision_loss,
	clippy::cast_sign_loss,
	clippy::dbg_macro,
	clippy::deprecated_cfg_attr,
	clippy::separated_literal_suffix,
	deprecated
)]
#![forbid(deprecated_in_future)]
#![allow(clippy::missing_errors_doc, clippy::module_name_repetitions)]

mod error;
mod message;
mod system;
mod task;
mod worker;

pub use error::{DispatcherShutdownError, RunError, SystemError as TaskSystemError};
pub use system::{
	BaseDispatcher as BaseTaskDispatcher, Dispatcher as TaskDispatcher, System as TaskSystem,
};
pub use task::{
	AnyTaskOutput, CancelTaskOnDrop, ExecStatus, Interrupter, InterrupterFuture, InterruptionKind,
	IntoAnyTaskOutput, IntoTask, SerializableTask, Task, TaskHandle, TaskId, TaskOutput,
	TaskRemoteController, TaskStatus,
};
