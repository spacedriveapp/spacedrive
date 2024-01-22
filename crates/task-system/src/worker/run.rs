use std::{
	collections::{HashMap, VecDeque},
	pin::pin,
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
	time::Duration,
};

use async_channel as chan;
use futures::StreamExt;
use futures_concurrency::{future::Race, stream::Merge};
use tokio::{
	spawn,
	sync::oneshot,
	task::{JoinError, JoinHandle},
	time::{interval_at, timeout, Instant},
};
use tokio_stream::wrappers::IntervalStream;
use tracing::{debug, error, warn};

use super::{
	super::{
		error::Error,
		message::WorkerMessage,
		system::SystemComm,
		task::{DynTask, ExecStatus, InternalTaskExecStatus, TaskId, TaskStatus, TaskWorkState},
	},
	WorkStealer, WorkerId,
};

const ONE_SECOND: Duration = Duration::from_secs(1);

pub(super) async fn run(
	id: WorkerId,
	system_comm: SystemComm,
	work_stealer: WorkStealer,
	msgs_rx: chan::Receiver<WorkerMessage>,
) {
	let mut tasks = VecDeque::new();
	let mut paused_tasks: HashMap<TaskId, TaskWorkState> = HashMap::new();
	let mut suspended_task = None;
	let mut priority_tasks = VecDeque::new();

	let mut last_requested_help = Instant::now();

	let mut idle_checker_interval = interval_at(Instant::now(), ONE_SECOND);
	idle_checker_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

	let mut is_idle = true;

	enum StreamMessage {
		Commands(WorkerMessage),
		TaskOutput((TaskId, Result<TaskRunnerOutput, Error>)),
		IdleCheck,
	}

	let (runner_tx, runner_rx) = chan::bounded(8);

	let mut abort_and_suspend_map = HashMap::with_capacity(8);
	let mut tasks_kinds = HashMap::with_capacity(64);

	let mut current_task_handle = None;

	let mut suspend_on_shutdown_rx = pin!(runner_rx.clone());

	let mut msg_stream = pin!((
		msgs_rx.map(StreamMessage::Commands),
		runner_rx.map(StreamMessage::TaskOutput),
		IntervalStream::new(idle_checker_interval).map(|_| StreamMessage::IdleCheck),
	)
		.merge());

	while let Some(msg) = msg_stream.next().await {
		match msg {
			// Worker messages
			StreamMessage::Commands(WorkerMessage::NewTask(task_work_state)) => {
				let new_kind =
					PendingTaskKind::with_priority(task_work_state.task.kind().with_priority());

				tasks_kinds.insert(task_work_state.task.id(), new_kind);

				if is_idle {
					let (task_id, handle) =
						spawn_task_runner(task_work_state, &mut abort_and_suspend_map, &runner_tx);

					current_task_handle = Some(RunningTask {
						task_id,
						kind: new_kind,
						handle,
					});

					// Doesn't need to report working back to system as it already registered
					// that we're not idle anymore when it dispatched the task to this worker
					is_idle = false;
				} else {
					let old_kind = current_task_handle
						.as_ref()
						.expect("Worker is not idle, but no task is running")
						.kind;

					match (new_kind, old_kind) {
						(PendingTaskKind::Priority, PendingTaskKind::Priority) => {
							priority_tasks.push_front(task_work_state);
						}
						(PendingTaskKind::Priority, PendingTaskKind::Normal) => {
							let RunningTask {
								task_id: old_task_id,
								handle,
								..
							} = current_task_handle.take().expect("we just checked");

							// We put the query at the top of the priority queue, so it will be
							// dispatched by the StreamMessage::TaskOutput handler below
							priority_tasks.push_front(task_work_state);

							if abort_and_suspend_map
								.remove(&old_task_id)
								.expect("we always store the abort and suspend signalers")
								.suspend_tx
								.send(())
								.is_err()
							{
								warn!("Task <id='{old_task_id}'> suspend channel closed before receiving suspend signal. \
								This probably happened because the task finished before we could suspend it.");
							}

							if let Err(e) = handle.await {
								error!("Task <id='{old_task_id}'> failed to join: {e:#?}");
							}
						}
						(_, _) => {
							tasks.push_back(task_work_state);
						}
					}

					let task_count = total_tasks(&priority_tasks, &suspended_task, &tasks);
					if task_count > work_stealer.workers_count()
						&& last_requested_help.elapsed() > ONE_SECOND
					{
						system_comm.request_help(id, task_count).await;
						last_requested_help = Instant::now();
					}
				}
			}

			StreamMessage::Commands(WorkerMessage::TaskCountRequest(tx)) => {
				if tx
					.send(total_tasks(&priority_tasks, &suspended_task, &tasks))
					.is_err()
				{
					warn!("Task count request channel closed before sending task count");
				}
			}

			StreamMessage::Commands(WorkerMessage::ResumeTask { task_id, ack }) => {
				if let Some(task) = paused_tasks.remove(&task_id) {
					task.worktable.set_resumed();
					tasks.push_back(task);
					if ack.send(Ok(())).is_err() {
						warn!("Resume task channel closed before sending ack");
					}
				} else if ack.send(Err(Error::TaskNotFound(task_id))).is_err() {
					warn!("Resume task channel closed before sending ack");
				}
			}

			StreamMessage::Commands(WorkerMessage::ForceAbortion { task_id, ack }) => {
				if let Some(AbortAndSuspendSignalers { abort_tx, .. }) =
					abort_and_suspend_map.remove(&task_id)
				{
					let (tx, rx) = oneshot::channel();

					if abort_tx.send(tx).is_err() {
						warn!("Force abortion channel closed before sending ack");
					} else if ack
						.send(match timeout(ONE_SECOND, rx).await {
							Ok(Ok(res)) => res,
							// If the sender was dropped, then the task finished before we could
							// abort it which is fine
							Ok(Err(_)) => Ok(()),
							Err(_) => Err(Error::TaskForcedAbortTimeout(task_id)),
						})
						.is_err()
					{
						warn!("Force abortion channel closed before sending ack");
					}
				} else if ack.send(Err(Error::TaskNotFound(task_id))).is_err() {
					warn!("Force abortion channel closed before sending ack");
				}
			}

			StreamMessage::Commands(WorkerMessage::ShutdownRequest(tx)) => {
				let all_tasks = if !is_idle {
					let suspended_tasks = if let Some(RunningTask {
						task_id, handle, ..
					}) = current_task_handle.take()
					{
						if abort_and_suspend_map
							.remove(&task_id)
							.expect("we always store the abort and suspend signalers")
							.suspend_tx
							.send(())
							.is_err()
						{
							warn!("Shutdown request channel closed before sending abort signal");
						}

						if let Err(e) = handle.await {
							error!("Task <id='{task_id}'> failed to join: {e:#?}");
						}

						runner_tx.close();
						let mut suspended_tasks = Vec::new();

						while let Some((task_id, res)) = suspend_on_shutdown_rx.next().await {
							match res {
								Ok(TaskRunnerOutput {
									task_work_state: TaskWorkState { task, done_tx, .. },
									status,
								}) => match status {
									InternalTaskExecStatus::Done => {
										if done_tx.send(Ok(TaskStatus::Done)).is_err() {
											warn!("Task done channel closed before sending done response for task <id='{task_id}'>");
										}
									}

									InternalTaskExecStatus::Cancelled => {
										if done_tx.send(Ok(TaskStatus::Cancelled)).is_err() {
											warn!("Task done channel closed before sending done response for task <id='{task_id}'>");
										}
									}

									InternalTaskExecStatus::Suspend
									| InternalTaskExecStatus::Paused => suspended_tasks.push(task),
								},
								Err(e) => {
									error!(
										"Task <id='{task_id}'> failed to suspend on shutdown: {e:#?}"
									);
								}
							}
						}

						suspended_tasks
					} else {
						Vec::new()
					};

					priority_tasks
						.into_iter()
						.map(|task_work_state| task_work_state.task)
						.chain(suspended_tasks.into_iter())
						.chain(
							paused_tasks
								.into_values()
								.map(|task_work_state| task_work_state.task),
						)
						.chain(
							tasks
								.into_iter()
								.map(|task_work_state| task_work_state.task),
						)
						.collect::<Vec<_>>()
				} else {
					Vec::new()
				};

				if tx.send(all_tasks).is_err() {
					warn!("Shutdown request channel closed before sending task list");
				}

				return;
			}

			StreamMessage::Commands(WorkerMessage::StealRequest(tx)) => {
				if let Some((kind, task)) =
					get_next_task(&mut priority_tasks, &mut suspended_task, &mut tasks)
				{
					let task_id = task.task.id();
					tasks_kinds.remove(&task_id);

					if let Err(Some(task)) = tx.send(Some(task)) {
						warn!("Steal request channel closed before sending task");
						match kind {
							PendingTaskKind::Normal => tasks.push_front(task),
							PendingTaskKind::Priority => priority_tasks.push_front(task),
							PendingTaskKind::Suspended => suspended_task = Some(task),
						}

						tasks_kinds.insert(task_id, kind);
					}
				} else if tx.send(None).is_err() {
					warn!("Steal request channel closed before sending task");
				}
			}

			StreamMessage::Commands(WorkerMessage::WakeUp) => {
				if is_idle {
					if let Some(task_work_state) = work_stealer.steal(id).await {
						let kind = if task_work_state.task.kind().with_priority() {
							PendingTaskKind::Priority
						} else {
							PendingTaskKind::Normal
						};

						let (task_id, handle) = spawn_task_runner(
							task_work_state,
							&mut abort_and_suspend_map,
							&runner_tx,
						);

						current_task_handle = Some(RunningTask {
							task_id,
							kind,
							handle,
						});
						is_idle = false;
						system_comm.working_report(id).await;
					} else {
						system_comm.idle_report(id).await;
					}
				}
			}

			// TaskOutput messages
			StreamMessage::TaskOutput((
				task_id,
				Ok(TaskRunnerOutput {
					task_work_state:
						TaskWorkState {
							task,
							worktable,
							done_tx,
							interrupter,
						},
					status,
				}),
			)) => {
				match status {
					InternalTaskExecStatus::Done => {
						worktable.set_completed();
						if done_tx.send(Ok(TaskStatus::Done)).is_err() {
							warn!("Task done channel closed before sending done response for task <id='{task_id}'>");
						}
					}
					InternalTaskExecStatus::Paused => {
						paused_tasks.insert(
							task_id,
							TaskWorkState {
								task,
								worktable,
								done_tx,
								interrupter,
							},
						);
					}
					InternalTaskExecStatus::Cancelled => {
						if done_tx.send(Ok(TaskStatus::Cancelled)).is_err() {
							warn!("Task done channel closed before sending cancelled response for task <id='{task_id}'>");
						}
					}
					InternalTaskExecStatus::Suspend => {
						suspended_task = Some(TaskWorkState {
							task,
							worktable,
							done_tx,
							interrupter,
						});
					}
				}

				dispatch_next_task(
					id,
					&mut is_idle,
					&mut abort_and_suspend_map,
					(&system_comm, &work_stealer),
					&runner_tx,
					(task_id, &mut current_task_handle),
					(&mut priority_tasks, &mut suspended_task, &mut tasks),
				)
				.await;
			}

			StreamMessage::TaskOutput((task_id, Err(e))) => {
				if matches!(e, Error::TaskAborted(_)) {
					debug!("Sucessfully aborted task <id='{task_id}'>");
				} else {
					error!("Task <id='{task_id}'> failed: {e:#?}");
				}

				dispatch_next_task(
					id,
					&mut is_idle,
					&mut abort_and_suspend_map,
					(&system_comm, &work_stealer),
					&runner_tx,
					(task_id, &mut current_task_handle),
					(&mut priority_tasks, &mut suspended_task, &mut tasks),
				)
				.await;
			}

			// Idle checking to steal some work
			StreamMessage::IdleCheck => {
				if is_idle {
					if let Some(task_work_state) = work_stealer.steal(id).await {
						let kind = if task_work_state.task.kind().with_priority() {
							PendingTaskKind::Priority
						} else {
							PendingTaskKind::Normal
						};

						let (task_id, handle) = spawn_task_runner(
							task_work_state,
							&mut abort_and_suspend_map,
							&runner_tx,
						);

						current_task_handle = Some(RunningTask {
							task_id,
							kind,
							handle,
						});

						is_idle = false;
						system_comm.working_report(id).await;
					}
				}
			}
		}
	}
}

struct AbortAndSuspendSignalers {
	abort_tx: oneshot::Sender<oneshot::Sender<Result<(), Error>>>,
	suspend_tx: oneshot::Sender<()>,
}

struct TaskRunnerOutput {
	task_work_state: TaskWorkState,
	status: InternalTaskExecStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PendingTaskKind {
	Normal,
	Priority,
	Suspended,
}

impl PendingTaskKind {
	fn with_priority(has_priority: bool) -> Self {
		if has_priority {
			Self::Priority
		} else {
			Self::Normal
		}
	}
}

struct RunningTask {
	task_id: TaskId,
	kind: PendingTaskKind,
	handle: JoinHandle<()>,
}

#[inline(always)]
async fn dispatch_next_task(
	worker_id: WorkerId,
	is_idle: &mut bool,
	abort_and_suspend_map: &mut HashMap<TaskId, AbortAndSuspendSignalers>,
	(system_comm, work_stealer): (&SystemComm, &WorkStealer),
	runner_tx: &chan::Sender<(TaskId, Result<TaskRunnerOutput, Error>)>,
	(task_id, current_task_handle): (TaskId, &mut Option<RunningTask>),
	(priority_tasks, suspended_task, tasks): (
		&mut VecDeque<TaskWorkState>,
		&mut Option<TaskWorkState>,
		&mut VecDeque<TaskWorkState>,
	),
) {
	abort_and_suspend_map.remove(&task_id);

	let RunningTask {
		task_id: old_task_id,

		handle,
		..
	} = current_task_handle
		.take()
		.expect("Task handle missing, but task output received");

	assert_eq!(task_id, old_task_id, "Task output id mismatch");

	if let Err(e) = handle.await {
		error!("Task <id='{old_task_id}'> failed to join: {e:#?}");
	}

	if let Some((kind, task_work_state)) = get_next_task(priority_tasks, suspended_task, tasks) {
		let (task_id, handle) =
			spawn_task_runner(task_work_state, abort_and_suspend_map, runner_tx);

		*current_task_handle = Some(RunningTask {
			task_id,
			kind,
			handle,
		});
	} else if let Some(task_work_state) = work_stealer.steal(worker_id).await {
		let kind = PendingTaskKind::with_priority(task_work_state.task.kind().with_priority());

		let (task_id, handle) =
			spawn_task_runner(task_work_state, abort_and_suspend_map, runner_tx);

		*current_task_handle = Some(RunningTask {
			task_id,
			kind,
			handle,
		});
	} else {
		*is_idle = true;
		system_comm.idle_report(worker_id).await;
	}
}

fn get_next_task(
	priority_tasks: &mut VecDeque<TaskWorkState>,
	suspended_task: &mut Option<TaskWorkState>,
	tasks: &mut VecDeque<TaskWorkState>,
) -> Option<(PendingTaskKind, TaskWorkState)> {
	if let Some(task) = priority_tasks.pop_front() {
		return Some((PendingTaskKind::Priority, task));
	}

	if let Some(task) = suspended_task.take() {
		task.interrupter.reset();
		return Some((PendingTaskKind::Suspended, task));
	}

	tasks
		.pop_front()
		.map(|task| (PendingTaskKind::Normal, task))
}

fn spawn_task_runner(
	task_work_state: TaskWorkState,
	abort_and_suspend_map: &mut HashMap<TaskId, AbortAndSuspendSignalers>,
	runner_tx: &chan::Sender<(TaskId, Result<TaskRunnerOutput, Error>)>,
) -> (TaskId, JoinHandle<()>) {
	let task_id = task_work_state.task.id();
	let (abort_tx, abort_rx) = oneshot::channel();
	let (suspend_tx, suspend_rx) = oneshot::channel();

	abort_and_suspend_map.insert(
		task_id,
		AbortAndSuspendSignalers {
			abort_tx,
			suspend_tx,
		},
	);

	(
		task_id,
		spawn(run_single_task(
			task_work_state,
			runner_tx.clone(),
			suspend_rx,
			abort_rx,
		)),
	)
}

async fn run_single_task(
	TaskWorkState {
		mut task,
		worktable,
		interrupter,
		done_tx,
	}: TaskWorkState,
	runner_tx: chan::Sender<(TaskId, Result<TaskRunnerOutput, Error>)>,
	suspend_rx: oneshot::Receiver<()>,
	abort_rx: oneshot::Receiver<oneshot::Sender<Result<(), Error>>>,
) {
	let task_id = task.id();

	worktable.set_started();

	let handle = spawn({
		let interrupter = Arc::clone(&interrupter);
		async move {
			let res = task.run(&interrupter).await;

			(task, res)
		}
	});

	let abort_handle = handle.abort_handle();

	let has_suspended = Arc::new(AtomicBool::new(false));

	let suspender_handle = spawn({
		let has_suspended = Arc::clone(&has_suspended);
		let worktable = Arc::clone(&worktable);
		async move {
			if suspend_rx.await.is_ok() {
				let (tx, rx) = oneshot::channel();

				// The interrupter only knows about Pause and Cancel commands, we use pause as
				// the suspend task feature should be invisible to the user
				worktable.pause(tx).await;

				if let Err(e) = rx
					.await
					.expect("Task suspend channel closed while task is running")
				{
					error!("Task <id='{task_id}'> failed to suspend: {e:#?}");
				}

				has_suspended.store(true, Ordering::Relaxed);
			}
		}
	});

	enum RaceOutput {
		Completed(Result<(DynTask, Result<ExecStatus, Error>), JoinError>),
		Abort(oneshot::Sender<Result<(), Error>>),
	}

	match (async { RaceOutput::Completed(handle.await) }, async move {
		RaceOutput::Abort(
			abort_rx
				.await
				.expect("Abort channel closed while task is running"),
		)
	})
		.race()
		.await
	{
		RaceOutput::Completed(Ok((task, res))) => {
			runner_tx
				.send((
					task_id,
					res.map(|status| {
						let mut status = status.into();
						if status == InternalTaskExecStatus::Paused
							&& has_suspended.load(Ordering::Relaxed)
						{
							status = InternalTaskExecStatus::Suspend;
						}

						TaskRunnerOutput {
							task_work_state: TaskWorkState {
								task,
								worktable,
								interrupter,
								done_tx,
							},
							status,
						}
					}),
				))
				.await
				.expect("Task runner channel closed while sending task output");
		}

		RaceOutput::Completed(Err(join_error)) => {
			error!("Task <id='{task_id}'> failed to join: {join_error:#?}",);
			if done_tx.send(Err(Error::TaskJoin(task_id))).is_err() {
				error!("Task done channel closed while sending join error response");
			}

			if runner_tx
				.send((task_id, Err(Error::TaskJoin(task_id))))
				.await
				.is_err()
			{
				error!("Task runner channel closed while sending join error response");
			}
		}

		RaceOutput::Abort(tx) => {
			abort_handle.abort();

			if done_tx.send(Err(Error::TaskAborted(task_id))).is_err() {
				error!("Task done channel closed while sending abort error response");
			}

			if runner_tx
				.send((task_id, Err(Error::TaskAborted(task_id))))
				.await
				.is_err()
			{
				error!("Task runner channel closed while sending abort error response");
			}

			if tx.send(Ok(())).is_err() {
				error!("Task abort channel closed while sending abort error response");
			}
		}
	}

	// if we received a suspend signal this abort will do nothing, as the task finished already
	suspender_handle.abort();
	if let Err(e) = suspender_handle.await {
		if e.is_panic() {
			error!("Task <id='{task_id}'> suspender critically failed: {e:#?}");
		}
	}
}

fn total_tasks(
	priority_tasks: &VecDeque<TaskWorkState>,
	suspended_task: &Option<TaskWorkState>,
	tasks: &VecDeque<TaskWorkState>,
) -> usize {
	priority_tasks.len() + if suspended_task.is_some() { 1 } else { 0 } + tasks.len()
}
