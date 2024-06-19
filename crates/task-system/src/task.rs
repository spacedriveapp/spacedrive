use std::{
	fmt,
	future::{Future, IntoFuture},
	pin::{pin, Pin},
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
	task::{Context, Poll},
	time::Duration,
};

use async_channel as chan;
use async_trait::async_trait;
use downcast_rs::{impl_downcast, Downcast};
use futures::StreamExt;
use tokio::{spawn, sync::oneshot};
use tracing::{error, instrument, trace, warn, Instrument};
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
pub trait AnyTaskOutput: Send + Downcast + 'static {}

impl fmt::Debug for Box<dyn AnyTaskOutput> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "<AnyTaskOutput>")
	}
}

impl_downcast!(AnyTaskOutput);

/// Blanket implementation for all types that implements `Send + 'static`
impl<T: Send + 'static> AnyTaskOutput for T {}

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
pub trait Task<E: RunError>: Downcast + Send + Sync + 'static {
	/// An unique identifier for the task, it will be used to identify the task on the system and also to the user.
	fn id(&self) -> TaskId;

	/// This method defines whether a task should run with priority or not. The task system has a mechanism
	/// to suspend non-priority tasks on any worker and run priority tasks ASAP. This is useful for tasks that
	/// are more important than others, like a task that should be concluded and show results immediately to the user,
	/// as thumbnails being generated for the current open directory or copy/paste operations.
	fn with_priority(&self) -> bool {
		false
	}

	/// Here we define if we want the task system to shutdown our task if it takes too long to finish. By default the
	/// task system will wait indefinitely for the task to finish, but if the user wants to have a timeout, they can
	/// return a [`Duration`] here and the task system will cancel the task if it takes longer than the specified time.
	fn with_timeout(&self) -> Option<Duration> {
		None
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

impl<E: RunError> fmt::Debug for Box<dyn Task<E>> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "<Task>")
	}
}

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

pin_project_lite::pin_project! {
	/// Intermediate struct to wait until a pause or a cancel commands are sent by the user.
	#[must_use = "`InterrupterFuture` does nothing unless polled"]
	pub struct InterrupterFuture<'recv> {
		#[pin]
		fut: chan::Recv<'recv, InterruptionRequest>,
	}
}

impl Future for InterrupterFuture<'_> {
	type Output = InterruptionKind;

	fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		let this = self.project();

		match this.fut.poll(cx) {
			Poll::Ready(Ok(InterruptionRequest { kind, ack })) => {
				trace!(?kind, "Running task received interruption request");
				if ack.send(()).is_err() {
					warn!("TaskInterrupter ack channel closed");
				}
				if let InternalInterruptionKind::Suspend(has_suspended) = &kind {
					has_suspended.store(true, Ordering::SeqCst);
				}

				let kind = kind.into();

				Poll::Ready(kind)
			}
			Poll::Ready(Err(chan::RecvError)) => {
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
		}
	}
}

/// A helper object that can be used to check if the user requested a pause or a cancel, so the task `run`
/// implementation can decide the appropriated moment to pause or cancel the task.
#[derive(Debug)]
pub struct Interrupter {
	interrupt_rx: chan::Receiver<InterruptionRequest>,
}

impl Drop for Interrupter {
	fn drop(&mut self) {
		if !self.interrupt_rx.is_closed() {
			self.close();
		}
	}
}

impl Interrupter {
	pub(crate) fn new(interrupt_tx: chan::Receiver<InterruptionRequest>) -> Self {
		Self {
			interrupt_rx: interrupt_tx,
		}
	}

	/// Check if the user requested a pause or a cancel, returning the kind of interruption that was requested
	/// in a non-blocking manner.
	pub fn try_check_interrupt(&self) -> Option<InterruptionKind> {
		if let Ok(InterruptionRequest { kind, ack }) = self.interrupt_rx.try_recv() {
			trace!(?kind, "Interrupter received interruption request");

			if let InternalInterruptionKind::Suspend(has_suspended) = &kind {
				has_suspended.store(true, Ordering::SeqCst);
			}

			let kind = kind.into();

			if ack.send(()).is_err() {
				warn!("TaskInterrupter ack channel closed");
			}

			Some(kind)
		} else {
			None
		}
	}

	pub(super) fn close(&self) {
		self.interrupt_rx.close();
		if !self.interrupt_rx.is_empty() {
			trace!("Pending interruption requests were not handled");
			spawn({
				let interrupt_rx = self.interrupt_rx.clone();

				async move {
					let mut interrupt_stream = pin!(interrupt_rx);

					while let Some(InterruptionRequest { kind, ack }) =
						interrupt_stream.next().await
					{
						trace!(
							?kind,
							"Interrupter received interruption request after task was completed"
						);
						ack.send(()).expect("Interrupter ack channel closed");
					}
				}
				.in_current_span()
			});
		}
	}
}

#[macro_export]
macro_rules! check_interruption {
	($interrupter:ident) => {
		let interrupter: &Interrupter = $interrupter;

		match interrupter.try_check_interrupt() {
			Some($crate::InterruptionKind::Cancel) => {
				::tracing::trace!("Task was canceled by the user");
				return Ok($crate::ExecStatus::Canceled);
			}
			Some($crate::InterruptionKind::Pause) => {
				::tracing::trace!("Task was paused by the user or suspended by the task system");
				return Ok($crate::ExecStatus::Paused);
			}
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
				::tracing::trace!("Task was canceled by the user");

				return Ok($crate::ExecStatus::Canceled);
			}
			Some($crate::InterruptionKind::Pause) => {
				*duration_accumulator += instant.elapsed();
				::tracing::trace!("Task was paused by the user or suspended by the task system");

				return Ok($crate::ExecStatus::Paused);
			}
			None => { /* Everything is Awesome! */ }
		}
	};
}

/// The kind of interruption that can be requested by the user, a pause or a cancel
#[derive(Debug, Clone, Copy)]
pub enum InterruptionKind {
	Pause,
	Cancel,
}

#[derive(Debug, Clone)]
enum InternalInterruptionKind {
	Pause,
	Suspend(Arc<AtomicBool>),
	Cancel,
}

impl From<InternalInterruptionKind> for InterruptionKind {
	fn from(kind: InternalInterruptionKind) -> Self {
		match kind {
			InternalInterruptionKind::Pause | InternalInterruptionKind::Suspend(_) => Self::Pause,
			InternalInterruptionKind::Cancel => Self::Cancel,
		}
	}
}

#[derive(Debug)]
pub struct InterruptionRequest {
	kind: InternalInterruptionKind,
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
	#[instrument(skip(self), fields(task_id = %self.task_id), err)]
	pub async fn pause(&self) -> Result<(), SystemError> {
		if self.worktable.is_finalized() {
			trace!("Task is finalized, will not pause");
			return Ok(());
		}

		let is_paused = self.worktable.is_paused.load(Ordering::Acquire);
		let is_canceled = self.worktable.has_canceled.load(Ordering::Acquire);
		let is_done = self.worktable.is_done.load(Ordering::Acquire);

		trace!(%is_canceled, %is_done, "Received pause command task");

		if !is_paused && !is_canceled && !is_done {
			if self.worktable.is_running.load(Ordering::Acquire) {
				let (tx, rx) = oneshot::channel();

				trace!("Task is running, sending pause request");

				self.worktable.pause(tx);

				rx.await.expect("Worker failed to ack pause request");
			} else {
				trace!("Task is not running, setting is_paused flag and communicating with system");
				self.worktable.is_paused.store(true, Ordering::Release);

				let (tx, rx) = oneshot::channel();

				self.system_comm.pause_not_running_task(
					self.task_id,
					Arc::clone(&self.worktable),
					tx,
				);

				return rx
					.await
					.expect("Worker failed to ack pause not running task request");
			}
		}

		Ok(())
	}

	/// Gracefully cancel the task at a safe point defined by the user using the [`Interrupter`]
	///
	/// # Panics
	///
	/// Will panic if the worker failed to ack the cancel request
	#[instrument(skip(self), fields(task_id = %self.task_id))]
	pub async fn cancel(&self) -> Result<(), SystemError> {
		if self.worktable.is_finalized() {
			trace!("Task is finalized, will not cancel");
			return Ok(());
		}

		let is_canceled = self.worktable.has_canceled();
		let is_done = self.worktable.is_done();

		trace!(%is_canceled, %is_done, "Received cancel command task");

		if !is_canceled && !is_done {
			if self.worktable.is_running() {
				let (tx, rx) = oneshot::channel();

				trace!("Task is running, sending cancel request");

				self.worktable.cancel(tx);

				rx.await.expect("Worker failed to ack cancel request");
			} else {
				trace!(
					"Task is not running, setting is_canceled flag and communicating with system"
				);
				self.worktable.has_canceled.store(true, Ordering::Release);

				let (tx, rx) = oneshot::channel();

				self.system_comm.cancel_not_running_task(
					self.task_id,
					Arc::clone(&self.worktable),
					tx,
				);

				return rx
					.await
					.expect("Worker failed to ack cancel not running task request");
			}
		}

		Ok(())
	}

	/// Forcefully abort the task, this can lead to corrupted data or inconsistent states, so use it with caution.
	///
	/// # Panics
	///
	/// Will panic if the worker failed to ack the forced abortion request
	#[instrument(skip(self), fields(task_id = %self.task_id), err)]
	pub async fn force_abortion(&self) -> Result<(), SystemError> {
		if self.worktable.is_finalized() {
			trace!("Task is finalized, will not force abortion");
			return Ok(());
		}
		trace!("Received force abortion command task");
		self.worktable.set_aborted();

		let (tx, rx) = oneshot::channel();

		self.system_comm
			.force_abortion(self.task_id, Arc::clone(&self.worktable), tx);

		rx.await
			.expect("Worker failed to ack force abortion request")
	}

	/// Marks the task to be resumed by the task system, the worker will start processing it if there is a slot
	/// available or will be enqueued otherwise.
	///
	/// # Panics
	///
	/// Will panic if the worker failed to ack the resume request
	#[instrument(skip(self), fields(task_id = %self.task_id), err)]
	pub async fn resume(&self) -> Result<(), SystemError> {
		if self.worktable.is_finalized() {
			trace!("Task is finalized, will not resume");
			return Ok(());
		}
		trace!("Received resume command task");

		let (tx, rx) = oneshot::channel();

		self.system_comm
			.resume_task(self.task_id, Arc::clone(&self.worktable), tx);

		rx.await.expect("Worker failed to ack resume request")
	}

	/// Verify if the task was already completed
	#[must_use]
	pub fn is_done(&self) -> bool {
		self.worktable.is_done()
			| self.worktable.has_shutdown()
			| self.worktable.has_aborted()
			| self.worktable.has_canceled()
			| self.worktable.has_failed()
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
	pub async fn pause(&self) -> Result<(), SystemError> {
		self.controller.pause().await
	}

	/// Gracefully cancel the task at a safe point defined by the user using the [`Interrupter`]
	pub async fn cancel(&self) -> Result<(), SystemError> {
		self.controller.cancel().await
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
pub struct CancelTaskOnDrop<E: RunError>(Option<TaskHandle<E>>);

impl<E: RunError> CancelTaskOnDrop<E> {
	/// Create a new `CancelTaskOnDrop` object with the given `TaskHandle`.
	#[must_use]
	pub const fn new(handle: TaskHandle<E>) -> Self {
		Self(Some(handle))
	}
}

impl<E: RunError> Future for CancelTaskOnDrop<E> {
	type Output = Result<TaskStatus<E>, SystemError>;

	fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		if let Some(handle) = self.0.as_mut() {
			match Pin::new(handle).poll(cx) {
				Poll::Ready(res) => {
					self.0 = None;
					Poll::Ready(res)
				}
				Poll::Pending => Poll::Pending,
			}
		} else {
			error!("tried to poll an already completed CancelTaskOnDrop future");
			Poll::Pending
		}
	}
}

impl<E: RunError> Drop for CancelTaskOnDrop<E> {
	fn drop(&mut self) {
		// FIXME: We should use async drop when it becomes stable
		if let Some(handle) = self.0.take() {
			spawn(async move { handle.cancel().await }.in_current_span());
		}
	}
}

#[derive(Debug)]
pub struct TaskWorktable {
	started: AtomicBool,
	is_running: AtomicBool,
	is_done: AtomicBool,
	is_paused: AtomicBool,
	has_canceled: AtomicBool,
	has_aborted: AtomicBool,
	has_shutdown: AtomicBool,
	has_failed: AtomicBool,
	interrupt_tx: chan::Sender<InterruptionRequest>,
	finalized: AtomicBool,
	current_worker_id: AtomicWorkerId,
}

impl TaskWorktable {
	pub fn new(worker_id: WorkerId, interrupt_tx: chan::Sender<InterruptionRequest>) -> Self {
		Self {
			started: AtomicBool::new(false),
			is_running: AtomicBool::new(false),
			is_done: AtomicBool::new(false),
			is_paused: AtomicBool::new(false),
			has_canceled: AtomicBool::new(false),
			has_aborted: AtomicBool::new(false),
			has_shutdown: AtomicBool::new(false),
			has_failed: AtomicBool::new(false),
			finalized: AtomicBool::new(false),
			interrupt_tx,
			current_worker_id: AtomicWorkerId::new(worker_id),
		}
	}

	#[inline]
	pub fn worker_id(&self) -> WorkerId {
		self.current_worker_id.load(Ordering::Acquire)
	}

	#[inline]
	pub fn change_worker(&self, new_worker_id: WorkerId) {
		self.current_worker_id
			.store(new_worker_id, Ordering::Release);
	}

	pub fn set_started(&self) {
		self.started.store(true, Ordering::Relaxed);
		self.is_running.store(true, Ordering::Relaxed);
	}

	pub fn set_completed(&self) {
		self.is_done.store(true, Ordering::Relaxed);
		self.is_running.store(false, Ordering::Relaxed);
	}

	pub fn set_canceled(&self) {
		self.has_canceled.store(true, Ordering::Relaxed);
		self.is_running.store(false, Ordering::Relaxed);
	}

	pub fn set_unpause(&self) {
		self.is_paused.store(false, Ordering::Relaxed);
	}

	pub fn set_aborted(&self) {
		self.has_aborted.store(true, Ordering::Relaxed);
		self.is_running.store(false, Ordering::Relaxed);
	}

	pub fn set_failed(&self) {
		self.has_failed.store(true, Ordering::Relaxed);
		self.is_running.store(false, Ordering::Relaxed);
	}

	pub fn set_shutdown(&self) {
		self.has_shutdown.store(true, Ordering::Relaxed);
		self.is_running.store(false, Ordering::Relaxed);
	}

	pub fn set_finalized(&self) {
		self.finalized.store(true, Ordering::Release);
	}

	pub fn pause(self: &Arc<Self>, outer_tx: oneshot::Sender<()>) {
		spawn({
			let this = Arc::clone(self);

			trace!("Sending pause signal to Interrupter object on task");

			async move {
				let (tx, rx) = oneshot::channel();

				if this
					.interrupt_tx
					.send(InterruptionRequest {
						kind: InternalInterruptionKind::Pause,
						ack: tx,
					})
					.await
					.is_ok()
				{
					rx.await.expect("Task failed to ack pause request");

					this.is_paused.store(true, Ordering::Release);
					this.is_running.store(false, Ordering::Release);
				}

				trace!("Sent pause signal to Interrupter object on task");

				outer_tx
					.send(())
					.expect("Worker channel closed trying to pause task");
			}
			.in_current_span()
		});
	}

	pub fn suspend(
		self: &Arc<Self>,
		outer_tx: oneshot::Sender<()>,
		has_suspended: Arc<AtomicBool>,
	) {
		trace!("Sending suspend signal to Interrupter object on task");
		spawn({
			let this = Arc::clone(self);

			async move {
				let (tx, rx) = oneshot::channel();

				if this
					.interrupt_tx
					.send(InterruptionRequest {
						kind: InternalInterruptionKind::Suspend(has_suspended),
						ack: tx,
					})
					.await
					.is_ok()
				{
					rx.await.expect("Task failed to ack suspend request");

					this.is_paused.store(true, Ordering::Release);
					this.is_running.store(false, Ordering::Release);
				}

				if outer_tx.send(()).is_err() {
					trace!("Task suspend channel closed trying to suspend task, maybe task manage to be completed");
				}
			}
			.in_current_span()
		});
	}

	pub fn cancel(self: &Arc<Self>, outer_tx: oneshot::Sender<()>) {
		trace!("Sending cancel signal to Interrupter object on task");
		spawn({
			let this = Arc::clone(self);
			async move {
				let (tx, rx) = oneshot::channel();

				if this
					.interrupt_tx
					.send(InterruptionRequest {
						kind: InternalInterruptionKind::Cancel,
						ack: tx,
					})
					.await
					.is_ok()
				{
					rx.await.expect("Task failed to ack cancel request");

					this.has_canceled.store(true, Ordering::Release);
					this.is_running.store(false, Ordering::Release);
				}

				outer_tx
					.send(())
					.expect("Worker channel closed trying to cancel task");
			}
			.in_current_span()
		});
	}

	pub fn is_done(&self) -> bool {
		self.is_done.load(Ordering::Acquire)
	}

	pub fn is_running(&self) -> bool {
		self.is_running.load(Ordering::Acquire)
	}

	pub fn is_paused(&self) -> bool {
		self.is_paused.load(Ordering::Acquire)
	}

	pub fn has_canceled(&self) -> bool {
		self.has_canceled.load(Ordering::Acquire)
	}

	pub fn has_failed(&self) -> bool {
		self.has_failed.load(Ordering::Acquire)
	}

	pub fn has_aborted(&self) -> bool {
		self.has_aborted.load(Ordering::Acquire)
	}

	pub fn has_shutdown(&self) -> bool {
		self.has_shutdown.load(Ordering::Acquire)
	}

	pub fn is_finalized(&self) -> bool {
		self.finalized.load(Ordering::Acquire)
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingTaskKind {
	Normal,
	Priority,
	Suspended,
}

impl PendingTaskKind {
	const fn with_priority(has_priority: bool) -> Self {
		if has_priority {
			Self::Priority
		} else {
			Self::Normal
		}
	}
}

pub struct TaskWorkState<E: RunError> {
	pub(crate) task: Box<dyn Task<E>>,
	pub(crate) worktable: Arc<TaskWorktable>,
	pub(crate) done_tx: PanicOnSenderDrop<E>,
	pub(crate) interrupter: Arc<Interrupter>,
}

impl<E: RunError> TaskWorkState<E> {
	#[inline]
	pub fn id(&self) -> TaskId {
		self.task.id()
	}

	#[inline]
	pub fn kind(&self) -> PendingTaskKind {
		PendingTaskKind::with_priority(self.task.with_priority())
	}
}

#[derive(Debug)]
pub struct PanicOnSenderDrop<E: RunError> {
	task_id: TaskId,
	maybe_done_tx: Option<oneshot::Sender<Result<TaskStatus<E>, SystemError>>>,
}

impl<E: RunError> PanicOnSenderDrop<E> {
	pub fn new(
		task_id: TaskId,
		done_tx: oneshot::Sender<Result<TaskStatus<E>, SystemError>>,
	) -> Self {
		Self {
			task_id,
			maybe_done_tx: Some(done_tx),
		}
	}

	pub fn send(
		mut self,
		res: Result<TaskStatus<E>, SystemError>,
	) -> Result<(), Result<TaskStatus<E>, SystemError>> {
		self.maybe_done_tx
			.take()
			.expect("tried to send a task output twice to the same task handle")
			.send(res)
	}
}

impl<E: RunError> Drop for PanicOnSenderDrop<E> {
	#[track_caller]
	fn drop(&mut self) {
		trace!(task_id = %self.task_id, "Dropping TaskWorkState");
		assert!(
			self.maybe_done_tx.is_none(),
			"TaskHandle done channel dropped before sending a result: {}",
			std::panic::Location::caller()
		);
		trace!(task_id = %self.task_id,
			"TaskWorkState successfully dropped"
		);
	}
}
