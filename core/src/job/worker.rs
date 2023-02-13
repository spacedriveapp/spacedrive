use crate::invalidate_query;
use crate::job::{JobReportUpdate, JobStatus};
use crate::library::LibraryContext;
use tokio::sync::broadcast;
use tracing::warn;

use super::JobReport;

/// TODO
pub struct WorkerContext {
	pub(super) report: JobReport,
	pub library_ctx: LibraryContext,
	pub(super) shutdown_tx: broadcast::Sender<()>,
}

impl WorkerContext {
	pub fn progress(&mut self, updates: Vec<JobReportUpdate>) {
		// protect against updates if job is not running
		if self.report.status != JobStatus::Running {
			warn!("Attempted to update job progress while job is not running");
			return;
		};

		for update in updates {
			match update {
				JobReportUpdate::TaskCount(task_count) => {
					self.report.task_count = task_count as i32;
				}
				JobReportUpdate::CompletedTaskCount(completed_task_count) => {
					self.report.completed_task_count = completed_task_count as i32;
				}
				JobReportUpdate::Message(message) => {
					self.report.message = message;
				}
				JobReportUpdate::SecondsElapsed(seconds) => {
					self.report.seconds_elapsed += seconds as i32;
				}
			}
		}

		// TODO: Copy the prototype sender level debounce onto this invalidate_query call to respect argument.

		invalidate_query!(self.library_ctx, "jobs.getRunning");
	}

	pub fn shutdown_rx(&self) -> broadcast::Receiver<()> {
		self.shutdown_tx.subscribe()
	}
}
