use super::jobs::{JobReport, JobReportUpdate, JobStatus};
use crate::{ClientQuery, CoreContext, CoreEvent, InternalEvent, Job};
use std::sync::Arc;
use tokio::sync::{
	mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
	Mutex,
};
// used to update the worker state from inside the worker thread
pub enum WorkerEvent {
	Progressed(Vec<JobReportUpdate>),
	Completed,
	Failed,
}

enum WorkerState {
	Pending(Box<dyn Job>, UnboundedReceiver<WorkerEvent>),
	Running,
}

#[derive(Clone)]
pub struct WorkerContext {
	pub uuid: String,
	pub core_ctx: CoreContext,
	pub sender: UnboundedSender<WorkerEvent>,
}

impl WorkerContext {
	pub fn progress(&self, updates: Vec<JobReportUpdate>) {
		self.sender
			.send(WorkerEvent::Progressed(updates))
			.unwrap_or(());
	}
}

// a worker is a dedicated thread that runs a single job
// once the job is complete the worker will exit
pub struct Worker {
	pub job_report: JobReport,
	state: WorkerState,
	worker_sender: UnboundedSender<WorkerEvent>,
}

impl Worker {
	pub fn new(job: Box<dyn Job>) -> Self {
		let (worker_sender, worker_receiver) = unbounded_channel();
		let uuid = uuid::Uuid::new_v4().to_string();

		Self {
			state: WorkerState::Pending(job, worker_receiver),
			job_report: JobReport::new(uuid),
			worker_sender,
		}
	}
	// spawns a thread and extracts channel sender to communicate with it
	pub async fn spawn(worker: Arc<Mutex<Self>>, ctx: &CoreContext) {
		// we capture the worker receiver channel so state can be updated from inside the worker
		let mut worker_mut = worker.lock().await;
		// extract owned job and receiver from Self
		let (job, worker_receiver) =
			match std::mem::replace(&mut worker_mut.state, WorkerState::Running) {
				WorkerState::Pending(job, worker_receiver) => {
					worker_mut.state = WorkerState::Running;
					(job, worker_receiver)
				},
				WorkerState::Running => unreachable!(),
			};
		let worker_sender = worker_mut.worker_sender.clone();
		let core_ctx = ctx.clone();

		worker_mut.job_report.status = JobStatus::Running;
		// spawn task to handle receiving events from the worker
		tokio::spawn(Worker::track_progress(
			worker.clone(),
			worker_receiver,
			ctx.clone(),
		));

		let uuid = worker_mut.job_report.id.clone();
		// spawn task to handle running the job
		tokio::spawn(async move {
			let worker_ctx = WorkerContext {
				uuid,
				core_ctx,
				sender: worker_sender,
			};
			// run the job
			let result = job.run(worker_ctx.clone()).await;

			if let Err(_) = result {
				worker_ctx.sender.send(WorkerEvent::Failed).unwrap_or(());
			} else {
				// handle completion
				worker_ctx.sender.send(WorkerEvent::Completed).unwrap_or(());
				worker_ctx
					.core_ctx
					.internal_sender
					.send(InternalEvent::JobComplete(worker_ctx.uuid.clone()))
					.unwrap_or(());
			}
		});
	}

	pub fn id(&self) -> String {
		self.job_report.id.to_owned()
	}

	async fn track_progress(
		worker: Arc<Mutex<Self>>,
		mut channel: UnboundedReceiver<WorkerEvent>,
		ctx: CoreContext,
	) {
		while let Some(command) = channel.recv().await {
			let mut worker = worker.lock().await;

			match command {
				WorkerEvent::Progressed(changes) => {
					for change in changes {
						match change {
							JobReportUpdate::TaskCount(task_count) => {
								worker.job_report.task_count = task_count as i64;
							},
							JobReportUpdate::CompletedTaskCount(completed_task_count) => {
								worker.job_report.completed_task_count =
									completed_task_count as i64;
							},
							JobReportUpdate::Message(message) => {
								worker.job_report.message = message;
							},
						}
					}
					ctx.emit(CoreEvent::InvalidateQuery(ClientQuery::JobGetRunning))
						.await;
				},
				WorkerEvent::Completed => {
					worker.job_report.status = JobStatus::Completed;
					ctx.emit(CoreEvent::InvalidateQuery(ClientQuery::JobGetRunning))
						.await;
					ctx.emit(CoreEvent::InvalidateQuery(ClientQuery::JobGetHistory))
						.await;
					break;
				},
				WorkerEvent::Failed => {
					worker.job_report.status = JobStatus::Failed;
					ctx.emit(CoreEvent::InvalidateQuery(ClientQuery::JobGetHistory))
						.await;
					break;
				},
			}
		}
	}
}
