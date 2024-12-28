use std::{
	ops::{Deref, DerefMut},
	path::Path,
	sync::{
		atomic::{AtomicU8, Ordering},
		Arc,
	},
};

use chrono::{DateTime, Utc};
use sd_core_library::Library;
use sd_core_library_sync::SyncManager;
use sd_core_node::Node;
use sd_core_shared_context::{JobContext, OuterContext, ProgressUpdate, Report, UpdateEvent};
use sd_core_shared_types::{core_event::CoreEvent, jobs::progress::JobProgressEvent};

use tokio::{spawn, sync::RwLock};
use tracing::{error, trace};
use uuid::Uuid;

/// A helper extension trait for easier downcasting
pub trait SyncManagerExt {
	/// Attempt to downcast to a concrete SyncManager type
	fn as_sync_manager(&self) -> Option<&Arc<SyncManager>>;
}

impl SyncManagerExt for Arc<dyn SyncManagerInterface> {
	fn as_sync_manager(&self) -> Option<&Arc<SyncManager>> {
		(**self).as_any().downcast_ref::<Arc<SyncManager>>()
	}
}

#[derive(Clone)]
pub struct NodeContext {
	pub node: Arc<Node>,
	pub library: Arc<Library>,
}

impl NodeContext {
	pub fn new(node: Arc<Node>, library: Arc<Library>) -> Self {
		Self {
			node,
			library: Arc::clone(&library),
		}
	}
}

impl OuterContext for NodeContext {
	fn library_id(&self) -> Uuid {
		self.library.id
	}

	fn db(&self) -> &Arc<sd_prisma::prisma::PrismaClient> {
		&self.library.db
	}

	fn sync_interface(&self) -> &Arc<dyn SyncManagerInterface> {
		&self.library.sync
	}

	fn sync(&self) -> Arc<dyn SyncManagerInterface> {
		Arc::clone(&self.library.sync)
	}

	fn invalidate_query(&self, query: &'static str) {
		self.node.invalidate_query(query);
	}

	fn query_invalidator(&self) -> impl Fn(&'static str) + Send + Sync {
		let node = Arc::clone(&self.node);
		move |query| node.invalidate_query(query)
	}

	fn report_update(&self, update: UpdateEvent) {
		match update {
			UpdateEvent::Progress(progress) => {
				trace!("Progress update: {:?}", progress);
				if let Err(e) = self.node.emit(progress) {
					error!("Failed to emit progress update: {}", e);
				}
			}
			UpdateEvent::InvalidateQuery(query) => self.invalidate_query(query),
		}
	}

	fn get_data_directory(&self) -> &Path {
		self.node.data_dir()
	}
}

#[derive(Clone)]
pub struct JobContextImpl<OuterCtx: OuterContext> {
	report: Arc<RwLock<Report>>,
	outer_ctx: OuterCtx,
	start_time: DateTime<Utc>,
	report_update_counter: Arc<AtomicU8>,
}

impl<OuterCtx: OuterContext> JobContext<OuterCtx> for JobContextImpl<OuterCtx> {
	fn new(report: Report, outer_ctx: OuterCtx) -> Self {
		Self {
			report: Arc::new(RwLock::new(report)),
			outer_ctx,
			start_time: Utc::now(),
			report_update_counter: Arc::new(AtomicU8::new(0)),
		}
	}

	fn progress(&self, updates: impl IntoIterator<Item = ProgressUpdate> + Send) {
		let mut report = match self.report.try_write() {
			Ok(report) => report,
			Err(_) => {
				error!("Failed to acquire write lock on report");
				return;
			}
		};

		let mut changed_phase = false;

		for update in updates {
			match update {
				ProgressUpdate::TaskCount(count) => {
					if let Ok(mut task_count) = report.task_count.try_write() {
						*task_count = count;
					}
				}
				ProgressUpdate::Message(message) => {
					if let Ok(mut msg) = report.message.try_write() {
						trace!(job_id = ?report.status, %message, "job message;");
						*msg = Some(message);
					}
				}
				ProgressUpdate::Phase(phase) => {
					if let Ok(mut p) = report.phase.try_write() {
						trace!(
							job_id = ?report.status,
							"changing phase: {:?} -> {phase};",
							*p
						);
						*p = Some(phase);
						changed_phase = true;
					}
				}
			}
		}

		// Calculate elapsed time
		let elapsed = Utc::now() - self.start_time;

		// Calculate remaining time based on task count
		let task_count = *report.task_count.try_read().unwrap_or(&0);
		let completed_task_count = *report.completed_task_count.try_read().unwrap_or(&0);

		// Calculate estimated completion time
		let remaining_task_count = task_count.saturating_sub(completed_task_count);
		let remaining_time_per_task = elapsed / (completed_task_count.max(1) as i32);
		let remaining_time = remaining_time_per_task * remaining_task_count as i32;

		// Update estimated completion time
		if let Ok(mut estimated_completion) = report.estimated_completion.try_write() {
			if let Some(completion_time) = Utc::now().checked_add_signed(remaining_time) {
				*estimated_completion = completion_time;
			}
		}

		let counter = self.report_update_counter.fetch_add(1, Ordering::AcqRel);

		// Debounce updates to avoid overwhelming the system
		if counter == 50 || counter == 0 || changed_phase {
			self.report_update_counter.store(1, Ordering::Release);

			// Emit progress update
			self.outer_ctx
				.report_update(UpdateEvent::Progress(JobProgressEvent {
					id: report.id,
					library_id: self.outer_ctx.library_id(),
					task_count,
					completed_task_count,
					message: report.message.try_read().unwrap_or_default().clone(),
					phase: report.phase.try_read().unwrap_or_default().clone(),
					estimated_completion: *report
						.estimated_completion
						.try_read()
						.unwrap_or(&Utc::now()),
					info: report.info.try_read().unwrap_or_default().clone(),
				}));

			// Spawn database update
			let report = Arc::clone(&self.report);
			let outer_ctx = self.outer_ctx.clone();
			spawn(async move {
				if let Ok(report) = report.try_read() {
					if let Err(e) =
						sd_prisma::queries::job::update_job_report(outer_ctx.db(), &report).await
					{
						error!(
							?e,
							"Failed to update job report on debounced job progress event;"
						);
					}
				}
			});
		}
	}

	fn report(&self) -> impl Deref<Target = Report> {
		self.report.read()
	}

	fn report_mut(&self) -> impl DerefMut<Target = Report> {
		self.report.write()
	}

	fn get_outer_ctx(&self) -> OuterCtx {
		self.outer_ctx.clone()
	}
}
