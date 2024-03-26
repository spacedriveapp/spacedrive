use futures_concurrency::future::Join;
use sd_task_system::TaskHandle;
use uuid::Uuid;

mod indexer;
mod job_system;

pub type JobId = Uuid;

pub use indexer::IndexerJob;
pub use job_system::{
	error::JobSystemError,
	job::{IntoJob, JobBuilder, JobOutputData},
	report::{Report as JobReport, ReportError as JobReportError},
	JobSystem,
};

use crate::Error;

async fn cancel_pending_tasks(pending_tasks: impl IntoIterator<Item = &TaskHandle<Error>> + Send) {
	pending_tasks
		.into_iter()
		.map(TaskHandle::cancel)
		.collect::<Vec<_>>()
		.join()
		.await;
}
