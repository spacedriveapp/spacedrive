use std::{
	fmt,
	future::{pending, Future, IntoFuture},
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
use futures::{stream::FuturesUnordered, StreamExt};
use futures_concurrency::future::{Join, Race};
use pin_project_lite::pin_project;
use tokio::sync::oneshot;
use tracing::{error, trace, warn};
use uuid::Uuid;

use super::{
	error::Error,
	system::SystemComm,
	worker::{AtomicWorkerId, WorkerId},
};

pub type TaskId = Uuid;

pub trait AnyTaskOutput: Send + fmt::Debug + Downcast + 'static {}

impl_downcast!(AnyTaskOutput);

impl<T: fmt::Debug + Send + 'static> AnyTaskOutput for T {}

pub trait IntoAnyTaskOutput {
	fn into_output(self) -> TaskOutput;
}

impl<T: AnyTaskOutput + 'static> IntoAnyTaskOutput for T {
	fn into_output(self) -> TaskOutput {
		TaskOutput::Out(Box::new(self))
	}
}

#[derive(Debug)]
pub enum TaskOutput {
	Out(Box<dyn AnyTaskOutput>),
	Empty,
}

#[derive(Debug)]
pub enum TaskStatus<E: TaskRunError> {
	Done(TaskOutput),
	Canceled,
	ForcedAbortion,
	Shutdown(Box<dyn Task<E>>),
	Error(E),
}

#[derive(Debug)]
pub enum ExecStatus {
	Done(TaskOutput),
	Paused,
	Canceled,
}

#[derive(Debug)]
pub(crate) enum InternalTaskExecStatus<E: TaskRunError> {
	Done(TaskOutput),
	Paused,
	Canceled,
	Suspend,
	Error(E),
}

impl<E: TaskRunError> From<Result<ExecStatus, E>> for InternalTaskExecStatus<E> {
	fn from(result: Result<ExecStatus, E>) -> Self {
		result
			.map(|status| match status {
				ExecStatus::Done(out) => Self::Done(out),
				ExecStatus::Paused => Self::Paused,
				ExecStatus::Canceled => Self::Canceled,
			})
			.unwrap_or_else(|e| Self::Error(e))
	}
}

pub trait IntoTask<E: TaskRunError> {
	fn into_task(self) -> Box<dyn Task<E>>;
}

impl<T: Task<E> + 'static, E: TaskRunError> IntoTask<E> for T {
	fn into_task(self) -> Box<dyn Task<E>> {
		Box::new(self)
	}
}

pub trait TaskRunError: std::error::Error + fmt::Debug + Send + Sync + 'static {}

impl<T: std::error::Error + fmt::Debug + Send + Sync + 'static> TaskRunError for T {}

#[async_trait]
pub trait Task<E: TaskRunError>: fmt::Debug + Downcast + Send + 'static {
	async fn run(&mut self, interrupter: &Interrupter) -> Result<ExecStatus, E>;

	fn with_priority(&self) -> bool {
		false
	}

	fn id(&self) -> TaskId;
}

impl_downcast!(Task<E> where E: TaskRunError);

pin_project! {
	pub struct InterrupterFuture<'recv> {
		#[pin]
		fut: Recv<'recv, InterruptionRequest>,
		has_interrupted: &'recv AtomicU8,
	}
}

impl Future for InterrupterFuture<'_> {
	type Output = InterruptionKind;

	fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		let this = self.project();

		match this.fut.poll(cx) {
			Poll::Ready(Ok(InterruptionRequest { kind, ack })) => {
				if ack.send(Ok(())).is_err() {
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

	pub fn try_check_interrupt(&self) -> Option<InterruptionKind> {
		if let Some(kind) = InterruptionKind::load(&self.has_interrupted) {
			Some(kind)
		} else if let Ok(InterruptionRequest { kind, ack }) = self.interrupt_rx.try_recv() {
			if ack.send(Ok(())).is_err() {
				warn!("TaskInterrupter ack channel closed");
			}

			self.has_interrupted.store(kind as u8, Ordering::Relaxed);

			Some(kind)
		} else {
			None
		}
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
pub(crate) struct InterruptionRequest {
	kind: InterruptionKind,
	ack: oneshot::Sender<Result<(), Error>>,
}

#[derive(Debug)]
pub struct TaskHandle<E: TaskRunError> {
	pub(crate) worktable: Arc<TaskWorktable>,
	pub(crate) done_rx: oneshot::Receiver<Result<TaskStatus<E>, Error>>,
	pub(crate) system_comm: SystemComm,
	pub(crate) task_id: TaskId,
}

impl<E: TaskRunError> Future for TaskHandle<E> {
	type Output = Result<TaskStatus<E>, Error>;

	fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		Pin::new(&mut self.done_rx)
			.poll(cx)
			.map(|res| res.expect("TaskHandle done channel unexpectedly closed"))
	}
}

impl<E: TaskRunError> TaskHandle<E> {
	pub fn task_id(&self) -> TaskId {
		self.task_id
	}

	/// Gracefully pause the task at a safe point defined by the user using the `TaskInterrupter`
	pub async fn pause(&self) -> Result<(), Error> {
		let is_paused = self.worktable.is_paused.load(Ordering::Relaxed);
		let is_canceled = self.worktable.is_canceled.load(Ordering::Relaxed);
		let is_done = self.worktable.is_done.load(Ordering::Relaxed);

		trace!("Received pause command task: <is_canceled={is_canceled}, is_done={is_done}>");

		if !is_paused && !is_canceled && !is_done {
			if self.worktable.is_running.load(Ordering::Relaxed) {
				let (tx, rx) = oneshot::channel();

				trace!("Task is running, sending pause request");

				self.worktable.pause(tx).await;

				rx.await.expect("Worker failed to ack pause request")?;
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

	/// Gracefully cancel the task at a safe point defined by the user using the `TaskInterrupter`
	pub async fn cancel(&self) -> Result<(), Error> {
		let is_canceled = self.worktable.is_canceled.load(Ordering::Relaxed);
		let is_done = self.worktable.is_done.load(Ordering::Relaxed);

		trace!("Received cancel command task: <is_canceled={is_canceled}, is_done={is_done}>");

		if !is_canceled && !is_done {
			if self.worktable.is_running.load(Ordering::Relaxed) {
				let (tx, rx) = oneshot::channel();

				trace!("Task is running, sending cancel request");

				self.worktable.cancel(tx).await;

				rx.await.expect("Worker failed to ack cancel request")?;
			} else {
				trace!("Task is not running, setting is_canceled flag");
				self.worktable.is_canceled.store(true, Ordering::Relaxed);
				return self
					.system_comm
					.cancel_not_running_task(
						self.task_id,
						self.worktable.current_worker_id.load(Ordering::Relaxed),
					)
					.await;
			}
		}

		Ok(())
	}

	pub async fn force_abortion(&self) -> Result<(), Error> {
		self.worktable.set_aborted();
		self.system_comm
			.force_abortion(
				self.task_id,
				self.worktable.current_worker_id.load(Ordering::Relaxed),
			)
			.await
	}

	pub async fn resume(&self) -> Result<(), Error> {
		self.system_comm
			.resume_task(
				self.task_id,
				self.worktable.current_worker_id.load(Ordering::Relaxed),
			)
			.await
	}
}

#[derive(Debug)]
pub(crate) struct TaskWorktable {
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

	pub async fn pause(&self, tx: oneshot::Sender<Result<(), Error>>) {
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

	pub async fn cancel(&self, tx: oneshot::Sender<Result<(), Error>>) {
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
pub(crate) struct TaskWorkState<E: TaskRunError> {
	pub(crate) task: Box<dyn Task<E>>,
	pub(crate) worktable: Arc<TaskWorktable>,
	pub(crate) done_tx: oneshot::Sender<Result<TaskStatus<E>, Error>>,
	pub(crate) interrupter: Arc<Interrupter>,
}

impl<E: TaskRunError> TaskWorkState<E> {
	pub fn change_worker(&self, new_worker_id: WorkerId) {
		self.worktable
			.current_worker_id
			.store(new_worker_id, Ordering::Relaxed);
	}
}

/// Util struct that handles the completion with erroring of multiple related tasks at once
#[derive(Debug)]
pub struct TaskHandlesBag<E: TaskRunError> {
	handles: Vec<TaskHandle<E>>,
}

impl<E: TaskRunError> TaskHandlesBag<E> {
	pub fn new(handles: Vec<TaskHandle<E>>) -> Self {
		Self { handles }
	}

	// /// Wait for all tasks to run to completion, but in case of a task error, return the error and
	// /// cancel of all other tasks
	// pub async fn try_wait_all_or_interrupt(
	// 	self,
	// 	interrupt_rx: oneshot::Receiver<InterruptionKind>,
	// ) -> Result<Option<Self>, Error> {
	// 	let mut futures = FuturesUnordered::from_iter(self.handles.into_iter());

	// 	enum RaceOutput {
	// 		Completed(Result<TaskStatus, Error>),
	// 		Interrupted(InterruptionKind),
	// 	}

	// 	match (
	// 		async {
	// 			while let Some(res) = futures.next().await {
	// 				if matches!(
	// 					&res,
	// 					&Ok(TaskStatus::Cancelled) | &Ok(TaskStatus::ForcedAbortion) | &Err(_)
	// 				) {
	// 					return RaceOutput::Completed(res);
	// 				}
	// 			}

	// 			RaceOutput::Completed(Ok(TaskStatus::Done))
	// 		},
	// 		async move {
	// 			if let Ok(kind) = interrupt_rx.await {
	// 				RaceOutput::Interrupted(kind)
	// 			} else {
	// 				// if the sender was dropped, we will never resolve this interrupt future, so we
	// 				// wait until all tasks completion
	// 				pending().await
	// 			}
	// 		},
	// 	)
	// 		.race()
	// 		.await
	// 	{
	// 		RaceOutput::Completed(Ok(TaskStatus::Done)) => Ok(None),

	// 		RaceOutput::Completed(Ok(TaskStatus::Cancelled)) => {
	// 			warn!("Cancelling all tasks due to a task being cancelled");
	// 			cancel_tasks(futures).await;
	// 			Ok(None)
	// 		}

	// 		RaceOutput::Completed(Ok(TaskStatus::ForcedAbortion)) => {
	// 			warn!("Cancelling all tasks due to a task being force aborted");
	// 			cancel_tasks(futures).await;
	// 			Ok(None)
	// 		}

	// 		RaceOutput::Completed(Err(e)) => {
	// 			cancel_tasks(futures).await;
	// 			Err(e)
	// 		}

	// 		RaceOutput::Interrupted(kind) => Ok(Some(Self {
	// 			handles: futures
	// 				.into_iter()
	// 				.map(|handle| {
	// 					#[allow(clippy::async_yields_async)]
	// 					async move {
	// 						if let Err(e) = match kind {
	// 							InterruptionKind::Pause => handle.pause().await,
	// 							InterruptionKind::Cancel => handle.cancel().await,
	// 						} {
	// 							error!("Failed to pause task: {e:#?}");
	// 						}

	// 						handle
	// 					}
	// 				})
	// 				.collect::<Vec<_>>()
	// 				.join()
	// 				.await,
	// 		})),
	// 	}
	// }

	/// Wait all tasks run to completion or pause/cancel when you wish
	pub async fn wait_all_or_interrupt(
		self,
		interrupt_rx: oneshot::Receiver<InterruptionKind>,
	) -> (Option<Self>, Vec<Result<TaskStatus<E>, Error>>) {
		let mut futures = FuturesUnordered::from_iter(self.handles.into_iter());

		enum RaceOutput {
			Completed,
			Interrupted(InterruptionKind),
		}

		let mut outputs = Vec::with_capacity(futures.len());

		if let RaceOutput::Interrupted(kind) = (
			async {
				while let Some(res) = futures.next().await {
					outputs.push(res);
				}

				RaceOutput::Completed
			},
			async move {
				if let Ok(kind) = interrupt_rx.await {
					RaceOutput::Interrupted(kind)
				} else {
					pending().await
				}
			},
		)
			.race()
			.await
		{
			(
				Some(Self {
					handles: futures
						.into_iter()
						.map(|handle| {
							#[allow(clippy::async_yields_async)]
							async move {
								if let Err(e) = match kind {
									InterruptionKind::Pause => handle.pause().await,
									InterruptionKind::Cancel => handle.cancel().await,
								} {
									error!("Failed to pause task: {e:#?}");
								}

								handle
							}
						})
						.collect::<Vec<_>>()
						.join()
						.await,
				}),
				outputs,
			)
		} else {
			(None, outputs)
		}
	}
}

// async fn cancel_tasks(handles: FuturesUnordered<TaskHandle<E>>) {
// 	handles
// 		.into_iter()
// 		.map(|handle| {
// 			#[allow(clippy::async_yields_async)]
// 			async move {
// 				if let Err(e) = handle.cancel().await {
// 					error!("Failed to cancel task: {e:#?}");
// 				}

// 				handle
// 			}
// 		})
// 		.collect::<Vec<_>>()
// 		.join()
// 		.await;
// }
