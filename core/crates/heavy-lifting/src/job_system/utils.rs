use crate::Error;

use sd_task_system::TaskHandle;

use futures_concurrency::future::Join;

pub async fn cancel_pending_tasks(
	pending_tasks: impl IntoIterator<Item = &TaskHandle<Error>> + Send,
) {
	pending_tasks
		.into_iter()
		.map(TaskHandle::cancel)
		.collect::<Vec<_>>()
		.join()
		.await;
}
