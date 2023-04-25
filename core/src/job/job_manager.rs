use crate::{
	invalidate_query,
	job::{worker::Worker, DynJob, Job, JobError, StatefulJob},
	library::Library,
	location::indexer::{indexer_job::IndexerJob, shallow_indexer_job::ShallowIndexerJob},
	object::{
		file_identifier::{
			file_identifier_job::FileIdentifierJob,
			shallow_file_identifier_job::ShallowFileIdentifierJob,
		},
		fs::{
			copy::FileCopierJob, cut::FileCutterJob, decrypt::FileDecryptorJob,
			delete::FileDeleterJob, encrypt::FileEncryptorJob, erase::FileEraserJob,
		},
		preview::{
			shallow_thumbnailer_job::ShallowThumbnailerJob, thumbnailer_job::ThumbnailerJob,
		},
		validation::validator_job::ObjectValidatorJob,
	},
	prisma::{job, node},
	util,
};

use std::{
	collections::{HashMap, HashSet, VecDeque},
	fmt::Debug,
	fmt::{Display, Formatter},
	sync::Arc,
	time::Duration,
};

use chrono::{DateTime, Utc};
use prisma_client_rust::Direction;
use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;
use tokio::{
	sync::{broadcast, mpsc, Mutex, RwLock},
	time::sleep,
};
use tracing::{debug, error, info};
use uuid::Uuid;

// db is single threaded, nerd
const MAX_WORKERS: usize = 1;

pub enum JobManagerEvent {
	IngestJob(Library, Box<dyn DynJob>),
}

#[derive(Error, Debug)]
pub enum JobManagerError {
	#[error("Tried to dispatch a job that is already running: Job <name='{name}', hash='{hash}'>")]
	AlreadyRunningJob { name: &'static str, hash: u64 },

	#[error("Failed to fetch job data from database: {0}")]
	Database(#[from] prisma_client_rust::QueryError),

	#[error("Job error: {0}")]
	Job(#[from] JobError),
}

impl From<JobManagerError> for rspc::Error {
	fn from(value: JobManagerError) -> Self {
		match value {
			JobManagerError::AlreadyRunningJob { .. } => Self::with_cause(
				rspc::ErrorCode::BadRequest,
				"Tried to spawn a job that is already running!".to_string(),
				value,
			),
			JobManagerError::Database(_) => Self::with_cause(
				rspc::ErrorCode::InternalServerError,
				"Error accessing the database".to_string(),
				value,
			),
			JobManagerError::Job(_) => Self::with_cause(
				rspc::ErrorCode::InternalServerError,
				"Job error".to_string(),
				value,
			),
		}
	}
}

/// JobManager handles queueing and executing jobs using the `DynJob`
/// Handling persisting JobReports to the database, pause/resuming, and
///
pub struct JobManager {
	current_jobs_hashes: RwLock<HashSet<u64>>,
	job_queue: RwLock<VecDeque<Box<dyn DynJob>>>,
	running_workers: RwLock<HashMap<Uuid, Arc<Mutex<Worker>>>>,
	internal_sender: mpsc::UnboundedSender<JobManagerEvent>,
	shutdown_tx: Arc<broadcast::Sender<()>>,
}

impl JobManager {
	pub fn new() -> Arc<Self> {
		let (shutdown_tx, _shutdown_rx) = broadcast::channel(1);
		let (internal_sender, mut internal_receiver) = mpsc::unbounded_channel();
		let this = Arc::new(Self {
			current_jobs_hashes: RwLock::new(HashSet::new()),
			job_queue: RwLock::new(VecDeque::new()),
			running_workers: RwLock::new(HashMap::new()),
			internal_sender,
			shutdown_tx: Arc::new(shutdown_tx),
		});

		let this2 = this.clone();
		tokio::spawn(async move {
			// FIXME: if this task crashes, the entire application is unusable
			while let Some(event) = internal_receiver.recv().await {
				match event {
					JobManagerEvent::IngestJob(library, job) => {
						this2.clone().dispatch_job(&library, job).await
					}
				}
			}
		});

		debug!("JobManager initialized");

		this
	}

	pub async fn ingest(
		self: Arc<Self>,
		library: &Library,
		job: Box<dyn DynJob>,
	) -> Result<(), JobManagerError> {
		let job_hash = job.hash();

		if self.current_jobs_hashes.read().await.contains(&job_hash) {
			return Err(JobManagerError::AlreadyRunningJob {
				name: job.name(),
				hash: job_hash,
			});
		}

		debug!(
			"Ingesting job: <name='{}', hash='{}'>",
			job.name(),
			job_hash
		);

		self.current_jobs_hashes.write().await.insert(job_hash);
		self.dispatch_job(library, job).await;
		Ok(())
	}

	pub async fn complete(self: Arc<Self>, library: &Library, job_id: Uuid, job_hash: u64) {
		// remove worker from running workers and from current jobs hashes
		self.current_jobs_hashes.write().await.remove(&job_hash);
		self.running_workers.write().await.remove(&job_id);
		// continue queue
		let job = self.job_queue.write().await.pop_front();
		if let Some(job) = job {
			// We can't directly execute `self.ingest` here because it would cause an async cycle.
			self.internal_sender
				.send(JobManagerEvent::IngestJob(library.clone(), job))
				.unwrap_or_else(|_| {
					error!("Failed to ingest job!");
				});
		}
	}

	pub async fn get_running(&self) -> Vec<JobReport> {
		let mut ret = vec![];

		for worker in self.running_workers.read().await.values() {
			let report = worker.lock().await.report();
			if !report.is_background {
				ret.push(report);
			}
		}
		ret
	}

	pub async fn get_history(library: &Library) -> Result<Vec<JobReport>, JobManagerError> {
		Ok(library
			.db
			.job()
			.find_many(vec![job::status::not(JobStatus::Running as i32)])
			.order_by(job::date_created::order(Direction::Desc))
			.take(100)
			.exec()
			.await?
			.into_iter()
			.map(JobReport::from)
			.filter(|report| !report.is_background)
			.collect())
	}

	pub async fn clear_all_jobs(library: &Library) -> Result<(), JobManagerError> {
		library.db.job().delete_many(vec![]).exec().await?;

		invalidate_query!(library, "jobs.getHistory");
		Ok(())
	}

	pub async fn clear_job(id: Uuid, library: &Library) -> Result<(), JobManagerError> {
		// unsure whether we should only delete jobs if marked as `JobStatus::Completed`?
		// took inspiration from `clear_all_jobs` for now though
		library
			.db
			.job()
			.delete(job::id::equals(id.as_bytes().to_vec()))
			.exec()
			.await?;

		invalidate_query!(library, "jobs.getHistory");
		Ok(())
	}

	pub fn shutdown_tx(&self) -> Arc<broadcast::Sender<()>> {
		Arc::clone(&self.shutdown_tx)
	}

	pub async fn pause(&self) {
		if !self.running_workers.read().await.is_empty() {
			self.shutdown_tx
				.send(())
				.expect("Failed to send shutdown signal");
		}

		loop {
			sleep(Duration::from_millis(50)).await;
			if self.running_workers.read().await.is_empty() {
				break;
			}
		}
	}

	pub async fn resume_jobs(self: Arc<Self>, library: &Library) -> Result<(), JobManagerError> {
		for root_paused_job_report in library
			.db
			.job()
			.find_many(vec![
				job::status::equals(JobStatus::Paused as i32),
				job::parent_id::equals(None), // only fetch top-level jobs, they will resume their children
			])
			.exec()
			.await?
			.into_iter()
			.map(JobReport::from)
		{
			let children_jobs = library
				.db
				.job()
				.find_many(vec![job::parent_id::equals(Some(
					root_paused_job_report.id.as_bytes().to_vec(),
				))])
				.order_by(job::action::order(Direction::Asc))
				.exec()
				.await?
				.into_iter()
				.map(|job_data| get_resumable_job(JobReport::from(job_data), VecDeque::new()))
				.collect::<Result<_, _>>()?;

			Arc::clone(&self)
				.dispatch_job(
					library,
					get_resumable_job(root_paused_job_report, children_jobs)?,
				)
				.await;
		}

		Ok(())
	}

	async fn dispatch_job(self: Arc<Self>, library: &Library, mut job: Box<dyn DynJob>) {
		// create worker to process job
		let mut running_workers = self.running_workers.write().await;
		if running_workers.len() < MAX_WORKERS {
			info!("Running job: {:?}", job.name());

			let job_report = job
				.report_mut()
				.take()
				.expect("critical error: missing job on worker");

			let job_id = job_report.id;

			let worker = Worker::new(job, job_report);

			let wrapped_worker = Arc::new(Mutex::new(worker));

			if let Err(e) = Worker::spawn(
				Arc::clone(&self),
				Arc::clone(&wrapped_worker),
				library.clone(),
			)
			.await
			{
				error!("Error spawning worker: {:?}", e);
			} else {
				running_workers.insert(job_id, wrapped_worker);
			}
		} else {
			debug!(
				"Queueing job: <name='{}', hash='{}'>",
				job.name(),
				job.hash()
			);
			self.job_queue.write().await.push_back(job);
		}
	}
}

#[derive(Debug)]
pub enum JobReportUpdate {
	TaskCount(usize),
	CompletedTaskCount(usize),
	Message(String),
}

#[derive(Debug, Serialize, Deserialize, Type, Clone)]
pub struct JobReport {
	pub id: Uuid,
	pub name: String,
	pub action: Option<String>,
	pub data: Option<Vec<u8>>,
	pub metadata: Option<serde_json::Value>,
	pub is_background: bool,

	pub created_at: Option<DateTime<Utc>>,
	pub started_at: Option<DateTime<Utc>>,
	pub completed_at: Option<DateTime<Utc>>,

	pub parent_id: Option<Uuid>,

	pub status: JobStatus,
	pub task_count: i32,
	pub completed_task_count: i32,

	pub message: String,
	// pub percentage_complete: f64,
}

impl Display for JobReport {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		write!(
			f,
			"Job <name='{}', uuid='{}'> {:#?}",
			self.name, self.id, self.status
		)
	}
}

// convert database struct into a resource struct
impl From<job::Data> for JobReport {
	fn from(data: job::Data) -> Self {
		Self {
			id: Uuid::from_slice(&data.id).expect("corrupted database"),
			is_background: get_background_info_by_job_name(&data.name),
			name: data.name,
			action: data.action,
			data: data.data,
			metadata: data.metadata.and_then(|m| {
				serde_json::from_slice(&m).unwrap_or_else(|e| -> Option<serde_json::Value> {
					error!("Failed to deserialize job metadata: {}", e);
					None
				})
			}),
			created_at: Some(data.date_created.into()),
			started_at: data.date_started.map(|d| d.into()),
			completed_at: data.date_completed.map(|d| d.into()),
			parent_id: data
				.parent_id
				.map(|id| Uuid::from_slice(&id).expect("corrupted database")),
			status: JobStatus::try_from(data.status).expect("corrupted database"),
			task_count: data.task_count,
			completed_task_count: data.completed_task_count,
			message: String::new(),
		}
	}
}

impl JobReport {
	pub fn new(uuid: Uuid, name: String) -> Self {
		Self {
			id: uuid,
			is_background: get_background_info_by_job_name(&name),
			name,
			action: None,
			created_at: None,
			started_at: None,
			completed_at: None,
			status: JobStatus::Queued,
			task_count: 0,
			data: None,
			metadata: None,
			parent_id: None,
			completed_task_count: 0,
			message: String::new(),
		}
	}

	pub fn new_with_action(uuid: Uuid, name: String, action: impl AsRef<str>) -> Self {
		let mut report = Self::new(uuid, name);
		report.action = Some(action.as_ref().to_string());
		report
	}

	pub fn new_with_parent(
		uuid: Uuid,
		name: String,
		parent_id: Uuid,
		action: Option<String>,
	) -> Self {
		let mut report = Self::new(uuid, name);
		report.parent_id = Some(parent_id);
		report.action = action;
		report
	}

	pub async fn create(&mut self, library: &Library) -> Result<(), JobError> {
		let now = Utc::now();
		self.created_at = Some(now);

		library
			.db
			.job()
			.create(
				self.id.as_bytes().to_vec(),
				self.name.clone(),
				node::id::equals(library.node_local_id),
				util::db::chain_optional_iter(
					[
						job::action::set(self.action.clone()),
						job::data::set(self.data.clone()),
						job::date_created::set(now.into()),
						job::date_started::set(self.started_at.map(|d| d.into())),
					],
					[self
						.parent_id
						.map(|id| job::parent::connect(job::id::equals(id.as_bytes().to_vec())))],
				),
			)
			.exec()
			.await?;
		Ok(())
	}

	pub async fn update(&mut self, library: &Library) -> Result<(), JobError> {
		library
			.db
			.job()
			.update(
				job::id::equals(self.id.as_bytes().to_vec()),
				vec![
					job::status::set(self.status as i32),
					job::data::set(self.data.clone()),
					job::metadata::set(serde_json::to_vec(&self.metadata).ok()),
					job::task_count::set(self.task_count),
					job::completed_task_count::set(self.completed_task_count),
					job::date_started::set(self.started_at.map(|v| v.into())),
					job::date_completed::set(self.completed_at.map(|v| v.into())),
				],
			)
			.exec()
			.await?;
		Ok(())
	}
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, Eq, PartialEq)]
pub enum JobStatus {
	Queued = 0,
	Running = 1,
	Completed = 2,
	Canceled = 3,
	Failed = 4,
	Paused = 5,
}

impl TryFrom<i32> for JobStatus {
	type Error = JobError;

	fn try_from(value: i32) -> Result<Self, Self::Error> {
		let s = match value {
			0 => Self::Queued,
			1 => Self::Running,
			2 => Self::Completed,
			3 => Self::Canceled,
			4 => Self::Failed,
			5 => Self::Paused,
			_ => return Err(JobError::InvalidJobStatusInt(value)),
		};

		Ok(s)
	}
}

#[macro_use]
mod macros {
	macro_rules! dispatch_call_to_job_by_name {
		($job_name:expr, T -> $call:expr, default = $default:block, jobs = [ $($job:ty),+ $(,)?]) => {{
			match $job_name {
				$(<$job as $crate::job::StatefulJob>::NAME => {
					type T = $job;
					$call
				},)+
				_ => $default
			}
		}};
	}
}

fn get_background_info_by_job_name(name: &str) -> bool {
	dispatch_call_to_job_by_name!(
		name,
		T -> <T as StatefulJob>::IS_BACKGROUND,
		default = {
			error!(
				"Unknown job name '{name}' at `is_background` check, will use `false` as a safe default"
			);
			false
		},
		jobs = [
			ThumbnailerJob,
			ShallowThumbnailerJob,
			IndexerJob,
			ShallowIndexerJob,
			FileIdentifierJob,
			ShallowFileIdentifierJob,
			ObjectValidatorJob,
			FileCutterJob,
			FileCopierJob,
			FileDeleterJob,
			FileEraserJob,
			FileEncryptorJob,
			FileDecryptorJob,
		]
	)
}

fn get_resumable_job(
	job_report: JobReport,
	next_jobs: VecDeque<Box<dyn DynJob>>,
) -> Result<Box<dyn DynJob>, JobManagerError> {
	dispatch_call_to_job_by_name!(
		job_report.name.as_str(),
		T -> Job::resume(job_report, T {}, next_jobs),
		default = {
			error!(
				"Unknown job type: {}, id: {}",
				job_report.name, job_report.id
			);
			Err(JobError::UnknownJobName(job_report.id, job_report.name))
		},
		jobs = [
			ThumbnailerJob,
			ShallowThumbnailerJob,
			IndexerJob,
			ShallowIndexerJob,
			FileIdentifierJob,
			ShallowFileIdentifierJob,
			ObjectValidatorJob,
			FileCutterJob,
			FileCopierJob,
			FileDeleterJob,
			FileEraserJob,
		]
	)
	.map_err(Into::into)
}
