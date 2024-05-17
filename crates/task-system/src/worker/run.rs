use std::pin::pin;

use async_channel as chan;
use futures::StreamExt;
use futures_concurrency::stream::Merge;
use tokio::time::{interval_at, Instant};
use tokio_stream::wrappers::IntervalStream;
use tracing::{debug, error, trace, warn};

use super::{
	super::{
		error::RunError,
		message::{StoleTaskMessage, TaskOutputMessage, WorkerMessage},
		system::SystemComm,
	},
	runner::Runner,
	WorkStealer, WorkerId, ONE_SECOND,
};

enum StreamMessage<E: RunError> {
	Commands(WorkerMessage<E>),
	Steal(Option<StoleTaskMessage<E>>),
	TaskOutput(TaskOutputMessage<E>),
	IdleCheck,
}

pub(super) async fn run<E: RunError>(
	id: WorkerId,
	system_comm: SystemComm,
	work_stealer: WorkStealer<E>,
	msgs_rx: chan::Receiver<WorkerMessage<E>>,
) {
	let (mut runner, stole_task_rx, task_output_rx) = Runner::new(id, work_stealer, system_comm);

	let mut idle_checker_interval = interval_at(Instant::now(), ONE_SECOND);
	idle_checker_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

	let mut msg_stream = pin!((
		msgs_rx.map(StreamMessage::Commands),
		stole_task_rx.map(StreamMessage::Steal),
		task_output_rx.map(StreamMessage::TaskOutput),
		IntervalStream::new(idle_checker_interval).map(|_| StreamMessage::IdleCheck),
	)
		.merge());

	while let Some(msg) = msg_stream.next().await {
		match msg {
			// Worker messages
			StreamMessage::Commands(WorkerMessage::NewTask(task_work_state)) => {
				let task_id = task_work_state.task_id();
				runner.abort_steal_task();
				trace!("New task received: <worker_id='{id}', task_id='{task_id}'>");
				runner.new_task(task_work_state).await;
				trace!("New task added: <worker_id='{id}', task_id='{task_id}'>");
			}

			StreamMessage::Commands(WorkerMessage::TaskCountRequest(tx)) => {
				if tx.send(runner.total_tasks()).is_err() {
					warn!("Task count request channel closed before sending task count");
				}
			}

			StreamMessage::Commands(WorkerMessage::ResumeTask { task_id, ack }) => {
				trace!("Resume task request received: <worker_id='{id}', task_id='{task_id}'>");
				if ack.send(runner.resume_task(task_id).await).is_err() {
					warn!("Resume task channel closed before sending ack");
				}
				trace!("Resumed task: <worker_id='{id}', task_id='{task_id}'>");
			}

			StreamMessage::Commands(WorkerMessage::PauseNotRunningTask { task_id, ack }) => {
				if ack.send(runner.pause_not_running_task(task_id)).is_err() {
					warn!("Resume task channel closed before sending ack");
				}
			}

			StreamMessage::Commands(WorkerMessage::CancelNotRunningTask { task_id, ack }) => {
				runner.cancel_not_running_task(&task_id);
				if ack.send(()).is_err() {
					warn!("Resume task channel closed before sending ack");
				}
			}

			StreamMessage::Commands(WorkerMessage::ForceAbortion { task_id, ack }) => {
				trace!(
					"Force abortion task request received: <worker_id='{id}', task_id='{task_id}'>"
				);
				if ack
					.send(runner.force_task_abortion(&task_id).await)
					.is_err()
				{
					warn!("Force abortion channel closed before sending ack");
				}
				trace!("Force aborted task: <worker_id='{id}', task_id='{task_id}'>");
			}

			StreamMessage::Commands(WorkerMessage::ShutdownRequest(tx)) => {
				return runner.shutdown(tx).await;
			}

			StreamMessage::Commands(WorkerMessage::StealRequest {
				ack,
				stolen_task_tx,
			}) => {
				trace!("Steal task request received: <worker_id='{id}'>");
				if ack
					.send(runner.steal_request(stolen_task_tx).await)
					.is_err()
				{
					debug!("Steal request attempt aborted before sending ack");
				}
				trace!("Steal task request completed: <worker_id='{id}'>");
			}

			StreamMessage::Commands(WorkerMessage::WakeUp) => runner.wake_up(),

			// Runner messages
			StreamMessage::TaskOutput(TaskOutputMessage(task_id, Ok(output))) => {
				trace!(
					"Process task output request received: <worker_id='{id}', task_id='{task_id}'>"
				);
				runner.process_task_output(&task_id, output).await;
				trace!("Processed task output: <worker_id='{id}', task_id='{task_id}'>");
			}

			StreamMessage::TaskOutput(TaskOutputMessage(task_id, Err(()))) => {
				error!("Task failed <worker_id='{id}', task_id='{task_id}'>");

				runner.clear_errored_task(task_id).await;
				trace!("Failed task cleared: <worker_id='{id}', task_id='{task_id}'>");
			}

			StreamMessage::Steal(maybe_stolen_task) => {
				let maybe_task_id = maybe_stolen_task
					.as_ref()
					.map(|StoleTaskMessage(task_work_state)| task_work_state.task_id());
				trace!("Received stolen task request: <worker_id='{id}', maybe_task_id={maybe_task_id:?}>");
				runner.process_stolen_task(maybe_stolen_task).await;
				trace!(
					"Processed stolen task: <worker_id='{id}', maybe_task_id={maybe_task_id:?}>"
				);
			}

			// Idle checking to steal some work
			StreamMessage::IdleCheck => runner.idle_check(),
		}
	}
}
