use std::sync::Arc;

use crate::{ClientQuery, CoreContext, CoreEvent, Job};
use tokio::sync::{
	mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
	Mutex,
};

use super::jobs::{JobReport, JobReportUpdate, JobStatus};

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
	pub core_ctx: CoreContext,
	pub sender: UnboundedSender<WorkerEvent>,
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

		println!("worker uuid: {}", &uuid);

		Self {
			state: WorkerState::Pending(job, worker_receiver),
			job_report: JobReport::new(uuid),
			worker_sender,
		}
	}
	// spawns a thread and extracts channel sender to communicate with it
	pub async fn spawn(worker: Arc<Mutex<Self>>, ctx: &CoreContext) {
		println!("spawning worker");
		// we capture the worker receiver channel so state can be updated from inside the worker
		let mut worker_mut = worker.lock().await;

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

		tokio::spawn(Worker::track_progress(
			worker.clone(),
			worker_receiver,
			ctx.clone(),
		));

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

	async fn track_progress(
		worker: Arc<Mutex<Self>>,
		mut channel: UnboundedReceiver<WorkerEvent>,
		ctx: CoreContext,
	) {
		println!("tracking progress");
		while let Some(command) = channel.recv().await {
			let mut worker = worker.lock().await;

			match command {
				WorkerEvent::Progressed(changes) => {
					println!("worker event: progressed");
					for change in changes {
						match change {
							JobReportUpdate::TaskCount(task_count) => {
								worker.job_report.task_count = task_count;
							},
							JobReportUpdate::CompletedTaskCount(completed_task_count) => {
								worker.job_report.completed_task_count =
									completed_task_count;
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
