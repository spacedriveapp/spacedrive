use crate::Error;

use sd_task_system::{TaskHandle, TaskStatus};

use futures::{stream::FuturesUnordered, StreamExt};
use futures_concurrency::future::Join;
use tracing::{error, trace};

pub async fn cancel_pending_tasks(pending_tasks: &mut FuturesUnordered<TaskHandle<Error>>) {
	pending_tasks
		.iter()
		.map(TaskHandle::cancel)
		.collect::<Vec<_>>()
		.join()
		.await;

	trace!(total_tasks = %pending_tasks.len(), "canceled all pending tasks, now waiting completion");

	while let Some(task_result) = pending_tasks.next().await {
		match task_result {
			Ok(TaskStatus::Done((task_id, _))) => trace!(
				%task_id,
				"tasks cancellation received a completed task;",
			),

			Ok(TaskStatus::Canceled | TaskStatus::ForcedAbortion | TaskStatus::Shutdown(_)) => {
				// Job canceled task
			}

			Ok(TaskStatus::Error(e)) => error!(%e, "job canceled an errored task;"),

			Err(e) => error!(%e, "task system failed to cancel a task;"),
		}
	}
}
