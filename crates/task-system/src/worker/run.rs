use std::pin::pin;

use async_channel as chan;
use futures::StreamExt;
use futures_concurrency::stream::Merge;
use tokio::time::{interval_at, Instant};
use tokio_stream::wrappers::IntervalStream;
use tracing::{debug, error, warn};

use super::{
	super::{
		error::RunError,
		message::{StoleTaskMessage, TaskOutputMessage, WorkerMessage},
		system::SystemComm,
	},
	runner::Runner,
	WorkStealer, WorkerId, ONE_SECOND,
};

pub(super) async fn run<E: RunError>(
	id: WorkerId,
	system_comm: SystemComm,
	work_stealer: WorkStealer<E>,
	msgs_rx: chan::Receiver<WorkerMessage<E>>,
) {
	enum StreamMessage<E: RunError> {
		Commands(WorkerMessage<E>),
		Steal(Option<StoleTaskMessage<E>>),
		TaskOutput(TaskOutputMessage<E>),
		IdleCheck,
	}

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
				runner.abort_steal_task();
				runner.new_task(task_work_state).await;
			}

			StreamMessage::Commands(WorkerMessage::TaskCountRequest(tx)) => {
				if tx.send(runner.total_tasks()).is_err() {
					warn!("Task count request channel closed before sending task count");
				}
			}

			StreamMessage::Commands(WorkerMessage::ResumeTask { task_id, ack }) => {
				if ack.send(runner.resume_task(task_id).await).is_err() {
					warn!("Resume task channel closed before sending ack");
				}
			}

			StreamMessage::Commands(WorkerMessage::PauseNotRunningTask { task_id, ack }) => {
				if ack.send(runner.pause_not_running_task(task_id)).is_err() {
					warn!("Resume task channel closed before sending ack");
				}
			}

			StreamMessage::Commands(WorkerMessage::CancelNotRunningTask { task_id, ack }) => {
				runner.cancel_not_running_task(task_id);
				if ack.send(()).is_err() {
					warn!("Resume task channel closed before sending ack");
				}
			}

			StreamMessage::Commands(WorkerMessage::ForceAbortion { task_id, ack }) => {
				if ack.send(runner.force_task_abortion(task_id).await).is_err() {
					warn!("Force abortion channel closed before sending ack");
				}
			}

			StreamMessage::Commands(WorkerMessage::ShutdownRequest(tx)) => {
				return runner.shutdown(tx).await;
			}

			StreamMessage::Commands(WorkerMessage::StealRequest {
				ack,
				stolen_task_tx,
			}) => {
				if ack
					.send(runner.steal_request(stolen_task_tx).await)
					.is_err()
				{
					debug!("Steal request attempt aborted before sending ack");
				}
			}

			StreamMessage::Commands(WorkerMessage::WakeUp) => runner.wake_up(),

			// Runner messages
			StreamMessage::TaskOutput(TaskOutputMessage(task_id, Ok(output))) => {
				runner.process_task_output(task_id, output).await;
			}

			StreamMessage::TaskOutput(TaskOutputMessage(task_id, Err(()))) => {
				error!("Task failed <worker_id='{id}', task_id='{task_id}'>");

				runner.clean_suspended_task(task_id);

				runner.dispatch_next_task(task_id).await;
			}

			StreamMessage::Steal(maybe_stolen_task) => {
				runner.process_stolen_task(maybe_stolen_task).await;
			}

			// Idle checking to steal some work
			StreamMessage::IdleCheck => runner.idle_check(),
		}
	}
}
