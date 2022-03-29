use std::fmt::Debug;
use thiserror::Error;

use crate::prisma;

pub mod jobs;
pub mod worker;

#[derive(Error, Debug)]
pub enum JobError {
	#[error("Failed to create job (job_id {job_id:?})")]
	CreateFailure { job_id: String },
	#[error("Database error")]
	DatabaseError(#[from] prisma::QueryError),
}

// pub struct JobContext {
// 	pub core_ctx: CoreContext,
// 	pub job_data: JobReport,
// }

// #[derive(Debug)]
// pub enum JobCommand {
// 	Create(Box<dyn Job>),
// 	Update { id: i32, data: JobUpdateEvent },
// 	Completed { id: i32 },
// }

// #[derive(Debug)]
// pub struct JobUpdateEvent {
// 	pub task_count: Option<i32>,
// 	pub completed_task_count: Option<i32>,
// 	pub message: Option<String>,
// }

// // a struct to handle the runtime and execution of jobs
// pub struct Jobs {
// 	pub job_sender_channel: Sender<JobCommand>,
// 	pub running_job: Mutex<Option<JobReport>>,
// }

// impl Jobs {
// 	pub fn new() -> (Self, mpsc::Receiver<JobCommand>) {
// 		let (job_sender, job_receiver) = mpsc::channel(100);
// 		(
// 			Self {
// 				job_sender_channel: job_sender,
// 				running_job: Mutex::new(None),
// 			},
// 			job_receiver,
// 		)
// 	}

// 	pub fn start(&self, ctx: CoreContext, mut job_receiver: mpsc::Receiver<JobCommand>) {
// 		// open a thread to handle job execution
// 		tokio::spawn(async move {
// 			// local memory for job queue
// 			let mut queued_jobs: Vec<(Box<dyn Job>, JobReport)> = vec![];

// 			loop {
// 				tokio::select! {
// 					// when job is received via message channel
// 					Some(request) = job_receiver.recv() => {
// 						match request {
// 							// create a new job
// 							JobCommand::Create(job) => {
// 								// create job report and save to database
// 								let mut report = JobReport::new();
// 								println!("Creating job: {:?} Metadata: {:?}", &job, &report);
// 								report.create(&ctx).await;
// 								// queue the job
// 								queued_jobs.push((job, report));

// 								let current_running_job = self.running_job.lock().await;

// 								if current_running_job.is_none() {
// 									// replace the running job mutex with this job
// 									let (current_job, current_report) = queued_jobs.pop().unwrap();
// 									current_running_job.replace(current_report);
// 									// push job id into running jobs vector
// 									let id = report.id;
// 									let ctx = ctx.clone();

// 									// open a dedicated blocking thread to run job
// 									tokio::task::spawn_blocking(move || {
// 										// asynchronously call run method
// 										let handle = tokio::runtime::Handle::current();
// 										let job_sender = ctx.job_sender.clone();

// 										handle.block_on(current_report.update(&ctx, None, Some(JobStatus::Running))).unwrap();
// 										handle.block_on(job.run(JobContext { core_ctx: ctx.clone(), job_data: current_report.clone() })).unwrap();

// 										job_sender.send(JobCommand::Completed { id }).unwrap();

// 									});
// 								}
// 							}
// 							// update a running job
// 							JobCommand::Update { id, data } => {
// 								let ctx = ctx.clone();
// 								// find running job in memory by id
// 								let running_job = get_job(&id).unwrap_or_else(|| panic!("Job not found"));
// 								// update job data
// 								running_job.update(&ctx, Some(data), None).await.unwrap();
// 								// emit event to invalidate client cache
// 								ctx.emit(CoreEvent::InvalidateQuery(ClientQuery::JobGetRunning)).await;
// 							},
// 							JobCommand::Completed { id } => {
// 								let ctx = ctx.clone();
// 								let running_job = get_job(&id).unwrap_or_else(|| panic!("Job not found"));
// 								running_job.update(&ctx, None, Some(JobStatus::Completed)).await.unwrap();
// 								ctx.emit(CoreEvent::InvalidateQuery(ClientQuery::JobGetRunning)).await;
// 								ctx.emit(CoreEvent::InvalidateQuery(ClientQuery::JobGetHistory)).await;

// 							}
// 						}
// 					}
// 				}
// 			}
// 		});
// 	}

// 	pub async fn handle_job_command(&mut self, job: JobCommand) {
// 		self.job_sender_channel.send(job).await.unwrap_or(());
// 	}
// }

// impl JobReport {
// 	pub fn new() -> Self {
// 		Self {
// 			id: 0,
// 			// client_id: 0,
// 			date_created: chrono::Utc::now(),
// 			date_modified: chrono::Utc::now(),
// 			status: JobStatus::Queued,
// 			task_count: 0,
// 			completed_task_count: 0,
// 			message: String::new(),
// 		}
// 	}
// 	pub async fn create(&mut self, ctx: &CoreContext) {
// 		// let config = client::get();
// 		let job = ctx
// 			.database
// 			.job()
// 			.create_one(
// 				prisma::Job::action().set(1),
// 				// prisma::Job::clients().link(prisma::Client::id().equals(config.client_uuid)),
// 				vec![],
// 			)
// 			.exec()
// 			.await;
// 		self.id = job.id;
// 	}
// 	pub async fn update(
// 		&mut self,
// 		ctx: &CoreContext,
// 		changes: Option<JobUpdateEvent>,
// 		status: Option<JobStatus>,
// 	) -> Result<()> {
// 		match changes {
// 			Some(changes) => {
// 				if changes.task_count.is_some() {
// 					self.task_count = changes.task_count.unwrap();
// 				}
// 				if changes.completed_task_count.is_some() {
// 					self.completed_task_count = changes.completed_task_count.unwrap();
// 				}
// 				if changes.message.is_some() {
// 					self.message = changes.message.unwrap();
// 				}
// 			},
// 			None => {},
// 		}
// 		if status.is_some() {
// 			self.status = status.unwrap();

// 			if self.status == JobStatus::Completed {
// 				ctx.database
// 					.job()
// 					.find_unique(prisma::Job::id().equals(self.id))
// 					.update(vec![
// 						prisma::Job::status().set(self.status.int_value()),
// 						prisma::Job::task_count().set(self.task_count),
// 						prisma::Job::completed_task_count().set(self.completed_task_count),
// 						prisma::Job::date_modified().set(chrono::Utc::now()),
// 					])
// 					.exec()
// 					.await;
// 			}
// 		}
// 		println!("JOB REPORT: {:?}", self);

// 		Ok(())
// 	}

// 	pub async fn get_running(ctx: &CoreContext) -> Result<Vec<JobReport>, JobError> {
// 		let db = &ctx.database;
// 		let jobs = db
// 			.job()
// 			.find_many(vec![prisma::Job::status().equals(JobStatus::Running.int_value())])
// 			.exec()
// 			.await;

// 		Ok(jobs.into_iter().map(|j| j.into()).collect())
// 	}

// 	pub async fn get_history(ctx: &CoreContext) -> Result<Vec<JobReport>, JobError> {
// 		let db = &ctx.database;
// 		let jobs = db
// 			.job()
// 			.find_many(vec![or(vec![
// 				prisma::Job::status().equals(JobStatus::Completed.int_value()),
// 				prisma::Job::status().equals(JobStatus::Canceled.int_value()),
// 				prisma::Job::status().equals(JobStatus::Queued.int_value()),
// 			])])
// 			.exec()
// 			.await;

// 		Ok(jobs.into_iter().map(|j| j.into()).collect())
// 	}
// }
