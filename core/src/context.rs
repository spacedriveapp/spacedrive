use crate::{api::CoreEvent, invalidate_query, library::Library, old_job::JobProgressEvent, Node};

use sd_core_heavy_lifting::{
	job_system::report::{Report, Status},
	OuterContext, ProgressUpdate, UpdateEvent,
};
use sd_core_sync::SyncManager;

use std::{
	ops::{Deref, DerefMut},
	sync::{
		atomic::{AtomicU8, Ordering},
		Arc,
	},
};

use chrono::{DateTime, Utc};
use tokio::{spawn, sync::RwLock};
use tracing::{error, trace};
use uuid::Uuid;

#[derive(Clone)]
pub struct NodeContext {
	pub node: Arc<Node>,
	pub library: Arc<Library>,
}

pub trait NodeContextExt: sealed::Sealed {
	fn library(&self) -> &Arc<Library>;
}

mod sealed {
	pub trait Sealed {}
}

impl sealed::Sealed for NodeContext {}

impl NodeContextExt for NodeContext {
	fn library(&self) -> &Arc<Library> {
		&self.library
	}
}

impl OuterContext for NodeContext {
	fn id(&self) -> Uuid {
		self.library.id
	}

	fn db(&self) -> &Arc<sd_prisma::prisma::PrismaClient> {
		&self.library.db
	}

	fn sync(&self) -> &SyncManager {
		&self.library.sync
	}

	fn invalidate_query(&self, query: &'static str) {
		invalidate_query!(self.library, query)
	}

	fn query_invalidator(&self) -> impl Fn(&'static str) + Send + Sync {
		|query| {
			invalidate_query!(self.library, query);
		}
	}

	fn report_update(&self, update: UpdateEvent) {
		// FIX-ME: Remove this conversion once we have a proper atomic updates system
		let event = match update {
			UpdateEvent::NewThumbnail { thumb_key } => CoreEvent::NewThumbnail { thumb_key },
			UpdateEvent::NewIdentifiedObjects { file_path_ids } => {
				CoreEvent::NewIdentifiedObjects { file_path_ids }
			}
		};
		self.node.emit(event);
	}

	fn get_data_directory(&self) -> &std::path::Path {
		&self.node.data_dir
	}
}

#[derive(Clone)]
pub struct JobContext<OuterCtx: OuterContext + NodeContextExt> {
	outer_ctx: OuterCtx,
	report: Arc<RwLock<Report>>,
	start_time: DateTime<Utc>,
	report_update_counter: Arc<AtomicU8>,
}

impl<OuterCtx: OuterContext + NodeContextExt> OuterContext for JobContext<OuterCtx> {
	fn id(&self) -> Uuid {
		self.outer_ctx.id()
	}

	fn db(&self) -> &Arc<sd_prisma::prisma::PrismaClient> {
		self.outer_ctx.db()
	}

	fn sync(&self) -> &SyncManager {
		self.outer_ctx.sync()
	}

	fn invalidate_query(&self, query: &'static str) {
		self.outer_ctx.invalidate_query(query);
	}

	fn query_invalidator(&self) -> impl Fn(&'static str) + Send + Sync {
		self.outer_ctx.query_invalidator()
	}

	fn report_update(&self, update: UpdateEvent) {
		self.outer_ctx.report_update(update);
	}

	fn get_data_directory(&self) -> &std::path::Path {
		self.outer_ctx.get_data_directory()
	}
}

impl<OuterCtx: OuterContext + NodeContextExt> sd_core_heavy_lifting::JobContext<OuterCtx>
	for JobContext<OuterCtx>
{
	fn new(report: Report, outer_ctx: OuterCtx) -> Self {
		Self {
			report: Arc::new(RwLock::new(report)),
			outer_ctx,
			start_time: Utc::now(),
			report_update_counter: Arc::new(AtomicU8::new(0)),
		}
	}

	async fn progress(&self, updates: impl IntoIterator<Item = ProgressUpdate> + Send) {
		let mut report = self.report.write().await;

		// protect against updates if job is not running
		if report.status != Status::Running {
			return;
		};

		let mut changed_phase = false;

		for update in updates {
			match update {
				ProgressUpdate::TaskCount(task_count) => {
					report.task_count = task_count as i32;
				}
				ProgressUpdate::CompletedTaskCount(completed_task_count) => {
					report.completed_task_count = completed_task_count as i32;
				}

				ProgressUpdate::Message(message) => {
					trace!(job_id = %report.id, %message, "job message;");
					report.message = message;
				}
				ProgressUpdate::Phase(phase) => {
					trace!(
						job_id = %report.id,
						"changing phase: {} -> {phase};",
						report.phase
					);
					report.phase = phase;
					changed_phase = true;
				}
			}
		}

		// Calculate elapsed time
		let elapsed = Utc::now() - self.start_time;

		// Calculate remaining time
		let task_count = report.task_count as usize;
		let completed_task_count = report.completed_task_count as usize;
		let remaining_task_count = task_count.saturating_sub(completed_task_count);

		// Adding 1 to avoid division by zero
		let remaining_time_per_task = elapsed / (completed_task_count + 1) as i32;

		let remaining_time = remaining_time_per_task * remaining_task_count as i32;

		// Update the report with estimated remaining time
		report.estimated_completion = Utc::now()
			.checked_add_signed(remaining_time)
			.unwrap_or(Utc::now());

		let library = self.outer_ctx.library();

		let counter = self.report_update_counter.fetch_add(1, Ordering::AcqRel);

		if counter == 50 || counter == 0 || changed_phase {
			self.report_update_counter.store(1, Ordering::Release);

			spawn({
				let db = Arc::clone(&library.db);
				let report = report.clone();
				async move {
					if let Err(e) = report.update(&db).await {
						error!(
							?e,
							"Failed to update job report on debounced job progress event;"
						);
					}
				}
			});
		}

		// emit a CoreEvent
		library.emit(CoreEvent::JobProgress(JobProgressEvent {
			id: report.id,
			library_id: library.id,
			task_count: report.task_count,
			completed_task_count: report.completed_task_count,
			info: report.info.clone(),
			estimated_completion: report.estimated_completion,
			phase: report.phase.clone(),
			message: report.message.clone(),
		}));
	}

	async fn report(&self) -> impl Deref<Target = Report> {
		Arc::clone(&self.report).read_owned().await
	}

	async fn report_mut(&self) -> impl DerefMut<Target = Report> {
		Arc::clone(&self.report).write_owned().await
	}

	fn get_outer_ctx(&self) -> OuterCtx {
		self.outer_ctx.clone()
	}
}
