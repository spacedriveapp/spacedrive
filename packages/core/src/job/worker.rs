use crate::{ClientQuery, CoreContext, CoreEvent, Job};
use dyn_clone::clone_box;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use super::jobs::{JobReport, JobReportUpdate, JobStatus};

// used to update the worker state from inside the worker thread
pub enum WorkerEvent {
	Progressed(Vec<JobReportUpdate>),
	Completed,
	Failed,
}

#[derive(Clone)]
pub struct WorkerContext {
	pub core_ctx: CoreContext,
	pub sender: UnboundedSender<WorkerEvent>,
}

// a worker is a dedicated thread that runs a single job
// once the job is complete the worker will exit
pub struct Worker {
	job: Box<dyn Job>,
	pub job_report: JobReport,
	worker_channel: (UnboundedSender<WorkerEvent>, UnboundedReceiver<WorkerEvent>),
}

impl Worker {
	pub fn new(job: Box<dyn Job>) -> Self {
		let uuid = uuid::Uuid::new_v4().to_string();
		println!("worker uuid: {}", &uuid);
		Self {
			job,
			job_report: JobReport::new(uuid),
			worker_channel: unbounded_channel(),
		}
	}
	// spawns a thread and extracts channel sender to communicate with it
	pub async fn spawn(&mut self, ctx: &CoreContext) {
		println!("spawning worker");
		// we capture the worker receiver channel so state can be updated from inside the worker
		let worker_sender = self.worker_channel.0.clone();
		let core_ctx = ctx.clone();

		let job = clone_box(&*self.job);

		self.track_progress(&ctx).await;

		tokio::spawn(async move {
			println!("new worker thread spawned");
			// this is provided to the job function and used to issue updates
			let worker_ctx = WorkerContext {
				core_ctx,
				sender: worker_sender,
			};

			let result = job.run(worker_ctx.clone()).await;

			if let Err(_) = result {
				worker_ctx.sender.send(WorkerEvent::Failed).unwrap_or(());
			}
		});
	}
	pub fn id(&self) -> String {
		self.job_report.id.to_owned()
	}
	async fn track_progress(&mut self, ctx: &CoreContext) {
		println!("tracking progress");
		self.job_report.status = JobStatus::Running;
		loop {
			tokio::select! {
				Some(command) = self.worker_channel.1.recv() => {
					match command {
						WorkerEvent::Progressed(changes) => {
							println!("worker event: progressed");
							for change in changes {
								match change {
									JobReportUpdate::TaskCount(task_count) => {
										self.job_report.task_count = task_count;
									},
									JobReportUpdate::CompletedTaskCount(completed_task_count) => {
										self.job_report.completed_task_count = completed_task_count;
									},
									JobReportUpdate::Message(message) => {
										self.job_report.message = message;
									},
								}
							}
							ctx.emit(CoreEvent::InvalidateQuery(ClientQuery::JobGetRunning)).await;
						},
						WorkerEvent::Completed => {
							self.job_report.status = JobStatus::Completed;
							ctx.emit(CoreEvent::InvalidateQuery(ClientQuery::JobGetRunning)).await;
							ctx.emit(CoreEvent::InvalidateQuery(ClientQuery::JobGetHistory)).await;
						},
						WorkerEvent::Failed => {
							self.job_report.status = JobStatus::Failed;
							ctx.emit(CoreEvent::InvalidateQuery(ClientQuery::JobGetHistory)).await;
						},
					}
				}
			}
		}
	}
}
