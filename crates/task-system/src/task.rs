use std::{
	fmt,
	future::{Future, IntoFuture},
	pin::Pin,
	sync::{
		atomic::{AtomicBool, AtomicU8, Ordering},
		Arc,
	},
	task::{Context, Poll},
};

use async_channel as chan;
use async_trait::async_trait;
use chan::{Recv, RecvError};
use downcast_rs::{impl_downcast, Downcast};
use tokio::{runtime::Handle, sync::oneshot};
use tracing::{trace, warn};
use uuid::Uuid;

use super::{
	error::{RunError, SystemError},
	system::SystemComm,
	worker::{AtomicWorkerId, WorkerId},
};

/// A unique identifier for a task using the [`uuid`](https://docs.rs/uuid) crate.
pub type TaskId = Uuid;

/// A trait that represents any kind of output that a task can return.
///
/// The user will downcast it to the concrete type that the task returns. Most of the time,
/// tasks will not return anything, so it isn't a costly abstraction, as only a heap allocation
/// is needed when the user wants to return a [`Box<dyn AnyTaskOutput>`].
pub trait AnyTaskOutput: Send + fmt::Debug + Downcast + 'static {}

impl_downcast!(AnyTaskOutput);

/// Blanket implementation for all types that implements `std::fmt::Debug + Send + 'static`
impl<T: fmt::Debug + Send + 'static> AnyTaskOutput for T {}

/// A helper trait to convert any type that implements [`AnyTaskOutput`] into a [`TaskOutput`], boxing it.
pub trait IntoAnyTaskOutput {
	fn into_output(self) -> TaskOutput;
}

/// Blanket implementation for all types that implements [`AnyTaskOutput`]
impl<T: AnyTaskOutput + 'static> IntoAnyTaskOutput for T {
	fn into_output(self) -> TaskOutput {
		TaskOutput::Out(Box::new(self))
	}
}

/// An enum representing whether a task returned anything or not.
#[derive(Debug)]
pub enum TaskOutput {
	Out(Box<dyn AnyTaskOutput>),
	Empty,
}

impl From<()> for TaskOutput {
	fn from((): ()) -> Self {
		Self::Empty
	}
}

/// An enum representing all possible outcomes for a task.
#[derive(Debug)]
pub enum TaskStatus<E: RunError> {
	/// The task has finished successfully and maybe has some output for the user.
	Done((TaskId, TaskOutput)),
	/// Task was gracefully cancelled by the user.
	Canceled,
	/// Task was forcefully aborted by the user.
	ForcedAbortion,
	/// The task system was shutdown and we give back the task to the user so they can downcast it
	/// back to the original concrete type and store it on disk or any other storage to be re-dispatched later.
	Shutdown(Box<dyn Task<E>>),
	/// Task had and error so we return it back and the user can handle it appropriately.
	Error(E),
}

/// Represents whether the current [`Task::run`] method on a task finished successfully or was interrupted.
///
/// `Done` and `Canceled` variants can only happen once, while `Paused` can happen multiple times,
/// whenever the user wants to pause the task.
#[derive(Debug)]
pub enum ExecStatus {
	Done(TaskOutput),
	Paused,
	Canceled,
}

#[derive(Debug)]
pub enum InternalTaskExecStatus<E: RunError> {
	Done(TaskOutput),
	Paused,
	Canceled,
	Suspend,
	Error(E),
}

impl<E: RunError> From<Result<ExecStatus, E>> for InternalTaskExecStatus<E> {
	fn from(result: Result<ExecStatus, E>) -> Self {
		result.map_or_else(Self::Error, |status| match status {
			ExecStatus::Done(out) => Self::Done(out),
			ExecStatus::Paused => Self::Paused,
			ExecStatus::Canceled => Self::Canceled,
		})
	}
}

/// A helper trait to convert any type that implements [`Task<E>`] into a [`Box<dyn Task<E>>`], boxing it.
pub trait IntoTask<E>: Send {
	fn into_task(self) -> Box<dyn Task<E>>;
}

/// Blanket implementation for all types that implements [`Task<E>`] and `'static`
impl<T: Task<E> + 'static, E: RunError> IntoTask<E> for T {
	fn into_task(self) -> Box<dyn Task<E>> {
		Box::new(self)
	}
}

/// The main trait that represents a task that can be dispatched to the task system.
///
/// All traits in the task system must return the same generic error type, so we can have a unified
/// error handling.
///
/// We're currently using the [`async_trait`](https://docs.rs/async-trait) crate to allow dyn async traits,
/// due to a limitation in the Rust language.
#[async_trait]
pub trait Task<E: RunError>: fmt::Debug + Downcast + Send + Sync + 'static {
	/// An unique identifier for the task, it will be used to identify the task on the system and also to the user.
	fn id(&self) -> TaskId;

	/// This method defines whether a task should run with priority or not. The task system has a mechanism
	/// to suspend non-priority tasks on any worker and run priority tasks ASAP. This is useful for tasks that
	/// are more important than others, like a task that should be concluded and show results immediately to the user,
	/// as thumbnails being generated for the current open directory or copy/paste operations.
	fn with_priority(&self) -> bool {
		false
	}

	/// This method represent the work that should be done by the worker, it will be called by the
	/// worker when there is a slot available in its internal queue.
	/// We receive a `&mut self` so any internal data can be mutated on each `run` invocation.
	///
	/// The [`interrupter`](Interrupter) is a helper object that can be used to check if the user requested a pause or a cancel,
	/// so the user can decide the appropriated moment to pause or cancel the task. Avoiding corrupted data or
	/// inconsistent states.
	async fn run(&mut self, interrupter: &Interrupter) -> Result<ExecStatus, E>;
}

impl_downcast!(Task<E> where E: RunError);

pub trait SerializableTask<E: RunError>: Task<E>
where
	Self: Sized,
{
	type SerializeError: std::error::Error + 'static;
	type DeserializeError: std::error::Error + 'static;
	type DeserializeCtx: 'static;

	fn serialize(self) -> impl Future<Output = Result<Vec<u8>, Self::SerializeError>> + Send;
	fn deserialize(
		data: &[u8],
		ctx: Self::DeserializeCtx,
	) -> impl Future<Output = Result<Self, Self::DeserializeError>> + Send;
}

/// Intermediate struct to wait until a pause or a cancel commands are sent by the user.
#[must_use = "`InterrupterFuture` does nothing unless polled"]
#[pin_project::pin_project]
pub struct InterrupterFuture<'recv> {
	#[pin]
	fut: Recv<'recv, InterruptionRequest>,
	has_interrupted: &'recv AtomicU8,
}

impl Future for InterrupterFuture<'_> {
	type Output = InterruptionKind;

	fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		let this = self.project();

		match this.fut.poll(cx) {
			Poll::Ready(Ok(InterruptionRequest { kind, ack })) => {
				if ack.send(()).is_err() {
					warn!("TaskInterrupter ack channel closed");
				}
				this.has_interrupted.store(kind as u8, Ordering::Relaxed);
				Poll::Ready(kind)
			}
			Poll::Ready(Err(RecvError)) => {
				// In case the task handle was dropped, we can't receive any more interrupt messages
				// so we will never interrupt and the task will run freely until ended
				warn!("Task interrupter channel closed, will run task until it finishes!");
				Poll::Pending
			}
			Poll::Pending => Poll::Pending,
		}
	}
}

/// We use an [`IntoFuture`] implementation to allow the user to use the `await` syntax on the [`Interrupter`] object.
/// With this trait, we return an [`InterrupterFuture`] that will await until the user requests a pause or a cancel.
impl<'recv> IntoFuture for &'recv Interrupter {
	type Output = InterruptionKind;

	type IntoFuture = InterrupterFuture<'recv>;

	fn into_future(self) -> Self::IntoFuture {
		InterrupterFuture {
			fut: self.interrupt_rx.recv(),
			has_interrupted: &self.has_interrupted,
		}
	}
}

/// A helper object that can be used to check if the user requested a pause or a cancel, so the task `run`
/// implementation can decide the appropriated moment to pause or cancel the task.
#[derive(Debug)]
pub struct Interrupter {
	interrupt_rx: chan::Receiver<InterruptionRequest>,
	has_interrupted: AtomicU8,
}

impl Interrupter {
	pub(crate) fn new(interrupt_tx: chan::Receiver<InterruptionRequest>) -> Self {
		Self {
			interrupt_rx: interrupt_tx,
			has_interrupted: AtomicU8::new(0),
		}
	}

	/// Check if the user requested a pause or a cancel, returning the kind of interruption that was requested
	/// in a non-blocking manner.
	pub fn try_check_interrupt(&self) -> Option<InterruptionKind> {
		InterruptionKind::load(&self.has_interrupted).map_or_else(
			|| {
				if let Ok(InterruptionRequest { kind, ack }) = self.interrupt_rx.try_recv() {
					if ack.send(()).is_err() {
						warn!("TaskInterrupter ack channel closed");
					}

					self.has_interrupted.store(kind as u8, Ordering::Relaxed);

					Some(kind)
				} else {
					None
				}
			},
			Some,
		)
	}

	pub(super) fn reset(&self) {
		self.has_interrupted
			.compare_exchange(
				InterruptionKind::Pause as u8,
				0,
				Ordering::Release,
				Ordering::Relaxed,
			)
			.expect("we must only reset paused tasks");
	}
}

#[macro_export]
macro_rules! check_interruption {
	($interrupter:ident) => {
		let interrupter: &Interrupter = $interrupter;

		match interrupter.try_check_interrupt() {
			Some($crate::InterruptionKind::Cancel) => return Ok($crate::ExecStatus::Canceled),
			Some($crate::InterruptionKind::Pause) => return Ok($crate::ExecStatus::Paused),
			None => { /* Everything is Awesome! */ }
		}
	};

	($interrupter:ident, $instant:ident, $duration_accumulator:ident) => {
		let interrupter: &Interrupter = $interrupter;
		let instant: Instant = $instant;
		let duration_accumulator: &mut Duration = $duration_accumulator;

		match interrupter.try_check_interrupt() {
			Some($crate::InterruptionKind::Cancel) => {
				*duration_accumulator += instant.elapsed();

				return Ok($crate::ExecStatus::Canceled);
			}
			Some($crate::InterruptionKind::Pause) => {
				*duration_accumulator += instant.elapsed();

				return Ok($crate::ExecStatus::Paused);
			}
			None => { /* Everything is Awesome! */ }
		}
	};
}

/// The kind of interruption that can be requested by the user, a pause or a cancel
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptionKind {
	Pause = 1,
	Cancel = 2,
}

impl InterruptionKind {
	fn load(kind: &AtomicU8) -> Option<Self> {
		match kind.load(Ordering::Relaxed) {
			1 => Some(Self::Pause),
			2 => Some(Self::Cancel),
			_ => None,
		}
	}
}

#[derive(Debug)]
pub struct InterruptionRequest {
	kind: InterruptionKind,
	ack: oneshot::Sender<()>,
}

/// A remote controller of a task that can be used to pause, cancel, resume, or force abortion.
#[derive(Debug, Clone)]
pub struct TaskRemoteController {
	pub(crate) worktable: Arc<TaskWorktable>,
	pub(crate) system_comm: SystemComm,
	pub(crate) task_id: TaskId,
}

impl TaskRemoteController {
	/// Get the unique identifier of the task
	#[must_use]
	pub const fn task_id(&self) -> TaskId {
		self.task_id
	}

	/// Gracefully pause the task at a safe point defined by the user using the [`Interrupter`]
	///
	/// # Panics
	///
	/// Will panic if the worker failed to ack the pause request
	pub async fn pause(&self) -> Result<(), SystemError> {
		let is_paused = self.worktable.is_paused.load(Ordering::Relaxed);
		let is_canceled = self.worktable.is_canceled.load(Ordering::Relaxed);
		let is_done = self.worktable.is_done.load(Ordering::Relaxed);

		trace!("Received pause command task: <is_canceled={is_canceled}, is_done={is_done}>");

		if !is_paused && !is_canceled && !is_done {
			if self.worktable.is_running.load(Ordering::Relaxed) {
				let (tx, rx) = oneshot::channel();

				trace!("Task is running, sending pause request");

				self.worktable.pause(tx).await;

				rx.await.expect("Worker failed to ack pause request");
			} else {
				trace!("Task is not running, setting is_paused flag");
				self.worktable.is_paused.store(true, Ordering::Relaxed);
				return self
					.system_comm
					.pause_not_running_task(
						self.task_id,
						self.worktable.current_worker_id.load(Ordering::Relaxed),
					)
					.await;
			}
		}

		Ok(())
	}

	/// Gracefully cancel the task at a safe point defined by the user using the [`Interrupter`]
	///
	/// # Panics
	///
	/// Will panic if the worker failed to ack the cancel request
	pub async fn cancel(&self) {
		let is_canceled = self.worktable.is_canceled.load(Ordering::Relaxed);
		let is_done = self.worktable.is_done.load(Ordering::Relaxed);

		trace!("Received cancel command task: <is_canceled={is_canceled}, is_done={is_done}>");

		if !is_canceled && !is_done {
			if self.worktable.is_running.load(Ordering::Relaxed) {
				let (tx, rx) = oneshot::channel();

				trace!("Task is running, sending cancel request");

				self.worktable.cancel(tx).await;

				rx.await.expect("Worker failed to ack cancel request");
			} else {
				trace!("Task is not running, setting is_canceled flag");
				self.worktable.is_canceled.store(true, Ordering::Relaxed);
				self.system_comm
					.cancel_not_running_task(
						self.task_id,
						self.worktable.current_worker_id.load(Ordering::Relaxed),
					)
					.await;
			}
		}
	}

	/// Forcefully abort the task, this can lead to corrupted data or inconsistent states, so use it with caution.
	pub async fn force_abortion(&self) -> Result<(), SystemError> {
		self.worktable.set_aborted();
		self.system_comm
			.force_abortion(
				self.task_id,
				self.worktable.current_worker_id.load(Ordering::Relaxed),
			)
			.await
	}

	/// Marks the task to be resumed by the task system, the worker will start processing it if there is a slot
	/// available or will be enqueued otherwise.
	pub async fn resume(&self) -> Result<(), SystemError> {
		self.system_comm
			.resume_task(
				self.task_id,
				self.worktable.current_worker_id.load(Ordering::Relaxed),
			)
			.await
	}

	/// Verify if the task was already completed
	#[must_use]
	pub fn is_done(&self) -> bool {
		self.worktable.is_done.load(Ordering::Relaxed)
	}
}

/// A handle returned when a task is dispatched to the task system, it can be used to pause, cancel, resume, or wait
/// until the task gets completed.
#[derive(Debug)]
pub struct TaskHandle<E: RunError> {
	pub(crate) done_rx: oneshot::Receiver<Result<TaskStatus<E>, SystemError>>,
	pub(crate) controller: TaskRemoteController,
}

impl<E: RunError> Future for TaskHandle<E> {
	type Output = Result<TaskStatus<E>, SystemError>;

	fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		Pin::new(&mut self.done_rx)
			.poll(cx)
			.map(|res| res.expect("TaskHandle done channel unexpectedly closed"))
	}
}

impl<E: RunError> TaskHandle<E> {
	/// Get the unique identifier of the task
	#[must_use]
	pub const fn task_id(&self) -> TaskId {
		self.controller.task_id
	}

	/// Gracefully pause the task at a safe point defined by the user using the [`Interrupter`]
	///
	/// # Panics
	///
	/// Will panic if the worker failed to ack the pause request
	pub async fn pause(&self) -> Result<(), SystemError> {
		self.controller.pause().await
	}

	/// Gracefully cancel the task at a safe point defined by the user using the [`Interrupter`]
	///
	/// # Panics
	///
	/// Will panic if the worker failed to ack the cancel request
	pub async fn cancel(&self) {
		self.controller.cancel().await;
	}

	/// Forcefully abort the task, this can lead to corrupted data or inconsistent states, so use it with caution.
	pub async fn force_abortion(&self) -> Result<(), SystemError> {
		self.controller.force_abortion().await
	}

	/// Marks the task to be resumed by the task system, the worker will start processing it if there is a slot
	/// available or will be enqueued otherwise.
	pub async fn resume(&self) -> Result<(), SystemError> {
		self.controller.resume().await
	}

	/// Gets the [`TaskRemoteController`] object that can be used to control the task remotely, to
	/// pause, cancel, resume, or force abortion.
	#[must_use]
	pub fn remote_controller(&self) -> TaskRemoteController {
		self.controller.clone()
	}
}

/// A helper struct when you just want to cancel a task if its `TaskHandle` gets dropped.
pub struct CancelTaskOnDrop<E: RunError>(pub TaskHandle<E>);

impl<E: RunError> Future for CancelTaskOnDrop<E> {
	type Output = Result<TaskStatus<E>, SystemError>;

	fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		Pin::new(&mut self.0).poll(cx)
	}
}

impl<E: RunError> Drop for CancelTaskOnDrop<E> {
	fn drop(&mut self) {
		// FIXME: We should use async drop when it becomes stable
		Handle::current().block_on(self.0.cancel());
	}
}

#[derive(Debug)]
pub struct TaskWorktable {
	started: AtomicBool,
	is_running: AtomicBool,
	is_done: AtomicBool,
	is_paused: AtomicBool,
	is_canceled: AtomicBool,
	is_aborted: AtomicBool,
	interrupt_tx: chan::Sender<InterruptionRequest>,
	current_worker_id: AtomicWorkerId,
}

impl TaskWorktable {
	pub fn new(worker_id: WorkerId, interrupt_tx: chan::Sender<InterruptionRequest>) -> Self {
		Self {
			started: AtomicBool::new(false),
			is_running: AtomicBool::new(false),
			is_done: AtomicBool::new(false),
			is_paused: AtomicBool::new(false),
			is_canceled: AtomicBool::new(false),
			is_aborted: AtomicBool::new(false),
			interrupt_tx,
			current_worker_id: AtomicWorkerId::new(worker_id),
		}
	}

	pub fn set_started(&self) {
		self.started.store(true, Ordering::Relaxed);
		self.is_running.store(true, Ordering::Relaxed);
	}

	pub fn set_completed(&self) {
		self.is_done.store(true, Ordering::Relaxed);
		self.is_running.store(false, Ordering::Relaxed);
	}

	pub fn set_unpause(&self) {
		self.is_paused.store(false, Ordering::Relaxed);
	}

	pub fn set_aborted(&self) {
		self.is_aborted.store(true, Ordering::Relaxed);
	}

	pub async fn pause(&self, tx: oneshot::Sender<()>) {
		self.is_paused.store(true, Ordering::Relaxed);
		self.is_running.store(false, Ordering::Relaxed);

		trace!("Sending pause signal to Interrupter object on task");

		self.interrupt_tx
			.send(InterruptionRequest {
				kind: InterruptionKind::Pause,
				ack: tx,
			})
			.await
			.expect("Worker channel closed trying to pause task");
	}

	pub async fn cancel(&self, tx: oneshot::Sender<()>) {
		self.is_canceled.store(true, Ordering::Relaxed);
		self.is_running.store(false, Ordering::Relaxed);

		self.interrupt_tx
			.send(InterruptionRequest {
				kind: InterruptionKind::Cancel,
				ack: tx,
			})
			.await
			.expect("Worker channel closed trying to pause task");
	}

	pub fn is_paused(&self) -> bool {
		self.is_paused.load(Ordering::Relaxed)
	}

	pub fn is_canceled(&self) -> bool {
		self.is_canceled.load(Ordering::Relaxed)
	}

	pub fn is_aborted(&self) -> bool {
		self.is_aborted.load(Ordering::Relaxed)
	}
}

#[derive(Debug)]
pub struct TaskWorkState<E: RunError> {
	pub(crate) task: Box<dyn Task<E>>,
	pub(crate) worktable: Arc<TaskWorktable>,
	pub(crate) done_tx: oneshot::Sender<Result<TaskStatus<E>, SystemError>>,
	pub(crate) interrupter: Arc<Interrupter>,
}

impl<E: RunError> TaskWorkState<E> {
	pub fn change_worker(&self, new_worker_id: WorkerId) {
		self.worktable
			.current_worker_id
			.store(new_worker_id, Ordering::Relaxed);
	}
}
