use std::pin::pin;

use async_channel as chan;
use futures::StreamExt;
use futures_concurrency::stream::Merge;
use tokio::time::{interval_at, Instant};
use tokio_stream::wrappers::IntervalStream;
use tracing::{error, warn};

use super::{
	super::{error::RunError, message::WorkerMessage, system::SystemComm},
	runner::Runner,
	RunnerMessage, WorkStealer, WorkerId, ONE_SECOND,
};

pub(super) async fn run<E: RunError>(
	id: WorkerId,
	system_comm: SystemComm,
	work_stealer: WorkStealer<E>,
	msgs_rx: chan::Receiver<WorkerMessage<E>>,
) {
	enum StreamMessage<E: RunError> {
		Commands(WorkerMessage<E>),
		RunnerMsg(RunnerMessage<E>),
		IdleCheck,
	}

	let (mut runner, runner_rx) = Runner::new(id, work_stealer, system_comm);

	let mut idle_checker_interval = interval_at(Instant::now(), ONE_SECOND);
	idle_checker_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

	let mut msg_stream = pin!((
		msgs_rx.map(StreamMessage::Commands),
		runner_rx.map(StreamMessage::RunnerMsg),
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
				if ack.send(Ok(())).is_err() {
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

			StreamMessage::Commands(WorkerMessage::StealRequest(tx)) => runner.steal_request(tx),

			StreamMessage::Commands(WorkerMessage::WakeUp) => runner.wake_up(),

			// Runner messages
			StreamMessage::RunnerMsg(RunnerMessage::TaskOutput(task_id, Ok(output))) => {
				runner.process_task_output(task_id, output).await;
			}

			StreamMessage::RunnerMsg(RunnerMessage::TaskOutput(task_id, Err(()))) => {
				error!("Task failed <worker_id='{id}', task_id='{task_id}'>");

				runner.clean_suspended_task(task_id);

				runner.dispatch_next_task(task_id).await;
			}

			StreamMessage::RunnerMsg(RunnerMessage::StoleTask(maybe_new_task)) => {
				runner.process_stolen_task(maybe_new_task).await;
			}

			// Idle checking to steal some work
			StreamMessage::IdleCheck => runner.idle_check(),
		}
	}
}
