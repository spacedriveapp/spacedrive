//! Job manager for scheduling and executing jobs
//! The job manager has its own database in the library directory, not the global library database.

use super::{
	context::CheckpointHandler,
	database::{self, JobDb},
	error::{JobError, JobResult},
	executor::JobExecutor,
	handle::JobHandle,
	progress::Progress,
	registry::REGISTRY,
	traits::{DynJob, Job, JobHandler, Resourceful},
	types::{JobId, JobInfo, JobPriority, JobStatus},
};
use crate::{
	context::CoreContext,
	infrastructure::events::{Event, EventBus},
	library::Library,
	operations::{
		files::copy::{FileCopyJob, MoveJob},
		indexing::IndexerJob,
		media::thumbnail::ThumbnailJob,
	},
};
use async_trait::async_trait;
use chrono::Utc;
use sd_task_system::{TaskDispatcher, TaskHandle, TaskSystem};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, DatabaseConnection, EntityTrait};
use std::{any::Any, collections::HashMap, path::PathBuf, sync::Arc};
use tokio::sync::{broadcast, mpsc, watch, Mutex, RwLock};
use tracing::{debug, error, info, warn};

/// Manages job execution for a library
pub struct JobManager {
	db: Arc<JobDb>,
	dispatcher: Arc<TaskSystem<JobError>>,
	running_jobs: Arc<RwLock<HashMap<JobId, RunningJob>>>,
	shutdown_tx: watch::Sender<bool>,
	context: Arc<CoreContext>,
	library_id: uuid::Uuid,
}

struct RunningJob {
	handle: JobHandle,
	task_handle: TaskHandle<JobError>,
	status_tx: watch::Sender<JobStatus>,
	latest_progress: Arc<Mutex<Option<Progress>>>,
}

impl JobManager {
	/// Create a new job manager
	pub async fn new(
		data_dir: PathBuf,
		context: Arc<CoreContext>,
		library_id: uuid::Uuid,
	) -> JobResult<Self> {
		// Initialize job database
		let job_db_path = data_dir.join("jobs.db");
		let db = database::init_database(&job_db_path).await?;

		// Create task system
		let dispatcher = TaskSystem::new();

		let (shutdown_tx, _) = watch::channel(false);

		let manager = Self {
			db: Arc::new(JobDb::new(db)),
			dispatcher: Arc::new(dispatcher),
			running_jobs: Arc::new(RwLock::new(HashMap::new())),
			shutdown_tx,
			context,
			library_id,
		};

		Ok(manager)
	}

	/// Initialize job manager (resume interrupted jobs)
	pub async fn initialize(&self) -> JobResult<()> {
		if let Err(e) = self.resume_interrupted_jobs().await {
			error!("Failed to resume interrupted jobs: {}", e);
		}
		Ok(())
	}

	/// Dispatch a job for execution
	pub async fn dispatch<J>(&self, job: J) -> JobResult<JobHandle>
	where
		J: Job + JobHandler,
	{
		self.dispatch_with_priority(job, JobPriority::NORMAL).await
	}

	/// Dispatch a job by name and parameters (useful for APIs)
	pub async fn dispatch_by_name(
		&self,
		job_name: &str,
		params: serde_json::Value,
	) -> JobResult<JobHandle> {
		self.dispatch_by_name_with_priority(job_name, params, JobPriority::NORMAL)
			.await
	}

	/// Dispatch a job by name with specific priority
	pub async fn dispatch_by_name_with_priority(
		&self,
		job_name: &str,
		params: serde_json::Value,
		priority: JobPriority,
	) -> JobResult<JobHandle> {
		// Check if job type is registered
		if !REGISTRY.has_job(job_name) {
			return Err(JobError::NotFound(format!(
				"Job type '{}' not registered",
				job_name
			)));
		}

		// Create job instance
		let erased_job = REGISTRY.create_job(job_name, params)?;

		let job_id = JobId::new();
		info!("Dispatching job {} ({}): {}", job_id, job_name, job_name);

		// Serialize job state for database
		let state = erased_job.serialize_state()?;

		// Create database record
		let job_model = database::jobs::ActiveModel {
			id: Set(job_id.to_string()),
			name: Set(job_name.to_string()),
			state: Set(state),
			status: Set(JobStatus::Queued.to_string()),
			priority: Set(priority.0),
			progress_type: Set(None),
			progress_data: Set(None),
			parent_job_id: Set(None),
			created_at: Set(Utc::now()),
			started_at: Set(None),
			completed_at: Set(None),
			paused_at: Set(None),
			error_message: Set(None),
			warnings: Set(None),
			non_critical_errors: Set(None),
			metrics: Set(None),
		};

		job_model.insert(self.db.conn()).await?;

		// Create channels
		let (status_tx, status_rx) = watch::channel(JobStatus::Queued);
		let (progress_tx, progress_rx) = mpsc::unbounded_channel();
		let (broadcast_tx, broadcast_rx) = broadcast::channel(100);

		// Create storage for latest progress
		let latest_progress = Arc::new(Mutex::new(None));

		// Create progress forwarding task
		let broadcast_tx_clone = broadcast_tx.clone();
		let latest_progress_clone = latest_progress.clone();
		let event_bus = self.context.events.clone();
		let job_id_clone = job_id.clone();
		let job_type_str = job_name.to_string();
		tokio::spawn(async move {
			let mut progress_rx: mpsc::UnboundedReceiver<Progress> = progress_rx;
			while let Some(progress) = progress_rx.recv().await {
				*latest_progress_clone.lock().await = Some(progress.clone());
				let _ = broadcast_tx_clone.send(progress.clone());

				// Emit enhanced progress event
				use crate::infrastructure::events::Event;

				// Extract generic progress data if available
				let generic_progress = match &progress {
					Progress::Structured(value) => {
						// Try to deserialize CopyProgress and convert to GenericProgress
						if let Ok(copy_progress) = serde_json::from_value::<
							crate::operations::files::copy::CopyProgress,
						>(value.clone())
						{
							use crate::infrastructure::jobs::generic_progress::ToGenericProgress;
							Some(serde_json::to_value(copy_progress.to_generic_progress()).ok())
						} else {
							None
						}
					}
					Progress::Generic(gp) => Some(serde_json::to_value(gp).ok()),
					_ => None,
				}
				.flatten();

				event_bus.emit(Event::JobProgress {
					job_id: job_id_clone.to_string(),
					job_type: job_type_str.to_string(),
					progress: progress.as_percentage().unwrap_or(0.0) as f64,
					message: Some(progress.to_string()),
					generic_progress,
				});
			}
		});

		// Get library from context using stored library_id
		let library = self
			.context
			.library_manager
			.get_library(self.library_id)
			.await
			.ok_or_else(|| {
				JobError::invalid_state(&format!("Library {} not found", self.library_id))
			})?;

		// Get services from context
		let networking = self.context.get_networking().await;
		let volume_manager = Some(self.context.volume_manager.clone());

		// Create executor using the erased job
		let executor = erased_job.create_executor(
			job_id,
			library,
			self.db.clone(),
			status_tx.clone(),
			progress_tx,
			broadcast_tx,
			Arc::new(DbCheckpointHandler {
				db: self.db.clone(),
			}),
			networking,
			volume_manager,
		);

		// Create handle
		let handle = JobHandle {
			id: job_id,
			task_handle: Arc::new(Mutex::new(None)),
			status_rx,
			progress_rx: broadcast_rx,
			output: Arc::new(Mutex::new(None)),
		};

		// Dispatch to task system
		let task_handle = self
			.dispatcher
			.get_dispatcher()
			.dispatch_boxed(executor)
			.await;

		match task_handle {
			Ok(handle_result) => {
				// Track running job
				self.running_jobs.write().await.insert(
					job_id,
					RunningJob {
						handle: handle.clone(),
						task_handle: handle_result,
						status_tx: status_tx.clone(),
						latest_progress,
					},
				);

				// Spawn a task to monitor job completion and clean up
				let running_jobs = self.running_jobs.clone();
				let job_id_clone = job_id.clone();
				let event_bus = self.context.events.clone();
				let job_type_str = job_name.to_string();
				tokio::spawn(async move {
					let mut status_rx = status_tx.subscribe();
					while status_rx.changed().await.is_ok() {
						let status = *status_rx.borrow();
						match status {
							JobStatus::Completed => {
								// Emit completion event
								event_bus.emit(Event::JobCompleted {
									job_id: job_id_clone.to_string(),
									job_type: job_type_str.clone(),
								});
								// Remove from running jobs
								running_jobs.write().await.remove(&job_id_clone);
								info!(
									"Job {} completed and removed from running jobs",
									job_id_clone
								);
								break;
							}
							JobStatus::Failed => {
								// Emit failure event
								event_bus.emit(Event::JobFailed {
									job_id: job_id_clone.to_string(),
									job_type: job_type_str.clone(),
									error: "Job failed".to_string(),
								});
								// Remove from running jobs
								running_jobs.write().await.remove(&job_id_clone);
								info!("Job {} failed and removed from running jobs", job_id_clone);
								break;
							}
							JobStatus::Cancelled => {
								// Emit cancellation event
								event_bus.emit(Event::JobCancelled {
									job_id: job_id_clone.to_string(),
									job_type: job_type_str.clone(),
								});
								// Remove from running jobs
								running_jobs.write().await.remove(&job_id_clone);
								info!(
									"Job {} cancelled and removed from running jobs",
									job_id_clone
								);
								break;
							}
							_ => {} // Continue monitoring for other status changes
						}
					}
				});

				Ok(handle)
			}
			Err(e) => Err(JobError::task_system(format!("{:?}", e))),
		}
	}

	/// Dispatch a job with specific priority
	pub async fn dispatch_with_priority<J>(
		&self,
		job: J,
		priority: JobPriority,
	) -> JobResult<JobHandle>
	where
		J: Job + JobHandler,
	{
		let job_id = JobId::new();
		info!("Dispatching job {}: {}", job_id, J::NAME);

		// Serialize job state
		let state =
			rmp_serde::to_vec(&job).map_err(|e| JobError::serialization(format!("{}", e)))?;

		// Create database record
		let job_model = database::jobs::ActiveModel {
			id: Set(job_id.to_string()),
			name: Set(J::NAME.to_string()),
			state: Set(state),
			status: Set(JobStatus::Queued.to_string()),
			priority: Set(priority.0),
			progress_type: Set(None),
			progress_data: Set(None),
			parent_job_id: Set(None),
			created_at: Set(Utc::now()),
			started_at: Set(None),
			completed_at: Set(None),
			paused_at: Set(None),
			error_message: Set(None),
			warnings: Set(None),
			non_critical_errors: Set(None),
			metrics: Set(None),
		};

		job_model.insert(self.db.conn()).await?;

		// Create channels
		let (status_tx, status_rx) = watch::channel(JobStatus::Queued);
		let (progress_tx, progress_rx) = mpsc::unbounded_channel();
		let (broadcast_tx, broadcast_rx) = broadcast::channel(100);

		// Create storage for latest progress
		let latest_progress = Arc::new(Mutex::new(None));

		// Create progress forwarding task with batching and throttling
		let broadcast_tx_clone = broadcast_tx.clone();
		let latest_progress_clone = latest_progress.clone();
		let event_bus = self.context.events.clone();
		let job_id_clone = job_id.clone();
		let job_type_str = J::NAME;
		let job_db_clone = self.db.clone();

		tokio::spawn(async move {
			let mut progress_rx: mpsc::UnboundedReceiver<Progress> = progress_rx;
			let mut last_db_update = std::time::Instant::now();
			const DB_UPDATE_INTERVAL: std::time::Duration = std::time::Duration::from_secs(2);

			while let Some(progress) = progress_rx.recv().await {
				// Store latest progress
				*latest_progress_clone.lock().await = Some(progress.clone());

				// Forward progress from mpsc to broadcast
				// Ignore errors if no one is listening
				let _ = broadcast_tx_clone.send(progress.clone());

				// Persist progress to database with throttling
				if last_db_update.elapsed() >= DB_UPDATE_INTERVAL {
					if let Err(e) = job_db_clone.update_progress(job_id_clone, &progress).await {
						debug!("Failed to persist job progress to database: {}", e);
					}
					last_db_update = std::time::Instant::now();
				}

				// Emit enhanced progress event
				use crate::infrastructure::events::Event;

				// Extract generic progress data if available
				let generic_progress = match &progress {
					Progress::Structured(value) => {
						// Try to deserialize CopyProgress and convert to GenericProgress
						if let Ok(copy_progress) = serde_json::from_value::<
							crate::operations::files::copy::CopyProgress,
						>(value.clone())
						{
							use crate::infrastructure::jobs::generic_progress::ToGenericProgress;
							Some(serde_json::to_value(copy_progress.to_generic_progress()).ok())
						} else {
							None
						}
					}
					Progress::Generic(gp) => Some(serde_json::to_value(gp).ok()),
					_ => None,
				}
				.flatten();

				event_bus.emit(Event::JobProgress {
					job_id: job_id_clone.to_string(),
					job_type: job_type_str.to_string(),
					progress: progress.as_percentage().unwrap_or(0.0) as f64,
					message: Some(progress.to_string()),
					generic_progress,
				});
			}

			// Final progress update when channel closes
			if let Some(final_progress) = &*latest_progress_clone.lock().await {
				if let Err(e) = job_db_clone
					.update_progress(job_id_clone, final_progress)
					.await
				{
					debug!("Failed to persist final job progress to database: {}", e);
				}
			}
		});

		// Get library from context using stored library_id
		let library = self
			.context
			.library_manager
			.get_library(self.library_id)
			.await
			.ok_or_else(|| {
				JobError::invalid_state(&format!("Library {} not found", self.library_id))
			})?;

		// Get services from context
		let networking = self.context.get_networking().await;
		let volume_manager = Some(self.context.volume_manager.clone());

		// Create executor
		let executor = JobExecutor::new(
			job,
			job_id,
			library,
			self.db.clone(),
			status_tx.clone(),
			progress_tx,
			broadcast_tx,
			Arc::new(DbCheckpointHandler {
				db: self.db.clone(),
			}),
			networking,
			volume_manager,
		);

		// Clone status_rx for cleanup task
		let status_rx_cleanup = status_rx.clone();

		// Create handle
		let handle = JobHandle {
			id: job_id,
			task_handle: Arc::new(Mutex::new(None)),
			status_rx,
			progress_rx: broadcast_rx,
			output: Arc::new(Mutex::new(None)),
		};

		// Dispatch to task system
		let task_handle = self.dispatcher.dispatch(executor).await;

		match task_handle {
			Ok(handle_result) => {
				// We don't store the task handle in JobHandle anymore
				// since it's already stored in RunningJob

				// Track running job
				self.running_jobs.write().await.insert(
					job_id,
					RunningJob {
						handle: handle.clone(),
						task_handle: handle_result,
						status_tx: status_tx.clone(),
						latest_progress: latest_progress.clone(),
					},
				);

				// Spawn a task to monitor job completion and clean up
				let running_jobs = self.running_jobs.clone();
				let job_id_clone = job_id.clone();
				let event_bus = self.context.events.clone();
				let job_type_str = J::NAME;
				tokio::spawn(async move {
					info!("Started cleanup monitor for job {}", job_id_clone);
					let mut status_monitor = status_rx_cleanup;
					while status_monitor.changed().await.is_ok() {
						let status = *status_monitor.borrow();
						info!("Job {} status changed to: {:?}", job_id_clone, status);
						match status {
							JobStatus::Completed => {
								// Emit completion event
								event_bus.emit(Event::JobCompleted {
									job_id: job_id_clone.to_string(),
									job_type: job_type_str.to_string(),
								});
								// Remove from running jobs
								running_jobs.write().await.remove(&job_id_clone);
								info!(
									"Job {} completed and removed from running jobs",
									job_id_clone
								);
								break;
							}
							JobStatus::Failed => {
								// Emit failure event
								event_bus.emit(Event::JobFailed {
									job_id: job_id_clone.to_string(),
									job_type: job_type_str.to_string(),
									error: "Job failed".to_string(),
								});
								// Remove from running jobs
								running_jobs.write().await.remove(&job_id_clone);
								info!("Job {} failed and removed from running jobs", job_id_clone);
								break;
							}
							JobStatus::Cancelled => {
								// Emit cancellation event
								event_bus.emit(Event::JobCancelled {
									job_id: job_id_clone.to_string(),
									job_type: job_type_str.to_string(),
								});
								// Remove from running jobs
								running_jobs.write().await.remove(&job_id_clone);
								info!(
									"Job {} cancelled and removed from running jobs",
									job_id_clone
								);
								break;
							}
							_ => {} // Continue monitoring for other status changes
						}
					}
				});

				Ok(handle)
			}
			Err(e) => Err(JobError::task_system(format!("{:?}", e))),
		}
	}

	/// Get a handle to a running job
	pub async fn get_job(&self, id: JobId) -> Option<JobHandle> {
		self.running_jobs
			.read()
			.await
			.get(&id)
			.map(|j| j.handle.clone())
	}

	/// List all available job types
	pub fn list_job_types(&self) -> Vec<&'static str> {
		REGISTRY.job_names()
	}

	/// Get schema for a job type
	pub fn get_job_schema(&self, job_name: &str) -> Option<super::types::JobSchema> {
		REGISTRY.get_schema(job_name)
	}

	/// Get a job instance by name and deserializing from raw state data
	/// This bypasses the JobExecutor wrapper to access the raw job for downcasting
	async fn get_raw_job_instance(
		&self,
		job_name: &str,
		job_state: &[u8],
	) -> JobResult<Box<dyn DynJob>> {
		// We need a way to get the raw job without the JobExecutor wrapper
		// For now, we'll implement a direct deserialization approach
		match job_name {
			"indexer" => {
				let job: IndexerJob = rmp_serde::from_slice(job_state).map_err(|e| {
					JobError::serialization(format!("Failed to deserialize IndexerJob: {}", e))
				})?;
				Ok(Box::new(job))
			}
			"thumbnail_generation" => {
				let job: ThumbnailJob = rmp_serde::from_slice(job_state).map_err(|e| {
					JobError::serialization(format!("Failed to deserialize ThumbnailJob: {}", e))
				})?;
				Ok(Box::new(job))
			}
			"file_copy" => {
				let job: FileCopyJob = rmp_serde::from_slice(job_state).map_err(|e| {
					JobError::serialization(format!("Failed to deserialize FileCopyJob: {}", e))
				})?;
				Ok(Box::new(job))
			}
			"move_files" => {
				let job: MoveJob = rmp_serde::from_slice(job_state).map_err(|e| {
					JobError::serialization(format!("Failed to deserialize MoveJob: {}", e))
				})?;
				Ok(Box::new(job))
			}
			_ => Err(JobError::NotFound(format!(
				"Unknown job type: {}",
				job_name
			))),
		}
	}

	/// List currently running jobs from memory (for live monitoring)
	pub async fn list_running_jobs(&self) -> Vec<JobInfo> {
		let running_jobs = self.running_jobs.read().await;
		let mut job_infos = Vec::new();

		for (job_id, running_job) in running_jobs.iter() {
			let handle = &running_job.handle;
			let status = handle.status();

			// Only include active jobs (running or paused)
			if status.is_active() {
				// Get latest progress
				let progress_percentage =
					if let Some(progress) = running_job.latest_progress.lock().await.as_ref() {
						progress.as_percentage().unwrap_or(0.0)
					} else {
						0.0
					};

				// Create JobInfo from in-memory state
				let job_info = JobInfo {
					id: job_id.0,
					name: format!("Job {}", job_id), // Use job ID as name for now
					status,
					progress: progress_percentage,
					started_at: chrono::Utc::now(), // TODO: Get actual start time
					completed_at: None,
					error_message: None,
					parent_job_id: None,
				};

				job_infos.push(job_info);
			}
		}

		job_infos
	}

	/// Find jobs that are processing specific entry IDs
	/// Uses downcasting to check if jobs implement Resourceful for precise resource tracking
	pub async fn find_jobs_affecting_entries(&self, entry_ids: &[i32]) -> JobResult<Vec<JobInfo>> {
		let active_jobs = self.list_jobs(Some(JobStatus::Running)).await?;
		let mut affecting_jobs = Vec::new();

		for job_info in active_jobs {
			// We need the full job state to check its resources.
			// This requires deserializing the job from the database.
			let job_instance = match self.get_job_data_for_resourceful_check(&job_info.id).await {
				Ok(instance) => instance,
				Err(e) => {
					// Log the error but continue; we don't want one bad job
					// to stop the entire state computation.
					tracing::warn!("Could not get instance for job {}: {}", job_info.id, e);
					continue;
				}
			};

			// Check if the job tracks specific resources
			if let Some(affected_resources) = job_instance.try_get_affected_resources() {
				// Check if there is any overlap between the job's resources
				// and the entries we are interested in.
				let is_affected = affected_resources.iter().any(|id| entry_ids.contains(id));

				if is_affected {
					affecting_jobs.push(job_info);
				}
			}
			// Jobs that don't track resources (return None) are skipped
		}
		Ok(affecting_jobs)
	}

	/// Helper method to get job data for resourceful checking
	pub async fn get_job_data_for_resourceful_check(
		&self,
		job_id: &Uuid,
	) -> JobResult<Box<dyn DynJob>> {
		// Load job from database
		let job_record = database::jobs::Entity::find_by_id(job_id.to_string())
			.one(self.db.conn())
			.await?
			.ok_or_else(|| JobError::NotFound(format!("Job {} not found", job_id)))?;

		// Use our helper method to deserialize the raw job
		self.get_raw_job_instance(&job_record.name, &job_record.state)
			.await
	}

	/// List all jobs with a specific status (unified query)
	pub async fn list_jobs(&self, status: Option<JobStatus>) -> JobResult<Vec<JobInfo>> {
		use sea_orm::QueryFilter;

		// First, get running jobs from memory for accurate real-time status
		let mut all_jobs = Vec::new();
		let running_jobs_map = self.running_jobs.read().await;

		// Collect job IDs that are in memory
		let mut in_memory_ids = std::collections::HashSet::new();

		for (job_id, running_job) in running_jobs_map.iter() {
			let handle = &running_job.handle;
			let current_status = handle.status();

			in_memory_ids.insert(job_id.0.to_string());

			// Check if status matches filter
			if let Some(filter_status) = status {
				if current_status != filter_status {
					continue;
				}
			}

			// Get latest progress from memory
			let progress_percentage =
				if let Some(progress) = running_job.latest_progress.lock().await.as_ref() {
					progress.as_percentage().unwrap_or(0.0)
				} else {
					0.0
				};

			// Get job name from database for complete info
			let job_name = match database::jobs::Entity::find_by_id(job_id.0.to_string())
				.one(self.db.conn())
				.await?
			{
				Some(db_job) => db_job.name,
				None => format!("Job {}", job_id.0),
			};

			all_jobs.push(JobInfo {
				id: job_id.0,
				name: job_name,
				status: current_status,
				progress: progress_percentage,
				started_at: chrono::Utc::now(), // TODO: Get from DB
				completed_at: None,
				error_message: None,
				parent_job_id: None,
			});
		}
		drop(running_jobs_map);

		// Now query database for jobs not in memory
		let mut query = database::jobs::Entity::find();

		if let Some(status) = status {
			use sea_orm::ColumnTrait;
			query = query.filter(database::jobs::Column::Status.eq(status.to_string()));
		}

		let db_jobs = query.all(self.db.conn()).await?;

		// Add database jobs that aren't in memory
		for j in db_jobs {
			// Skip if already in memory (memory takes precedence)
			if in_memory_ids.contains(&j.id) {
				continue;
			}

			let id = match j.id.parse::<Uuid>() {
				Ok(id) => id,
				Err(_) => continue,
			};

			let status = match j.status.as_str() {
				"queued" => JobStatus::Queued,
				"running" => JobStatus::Running,
				"paused" => JobStatus::Paused,
				"completed" => JobStatus::Completed,
				"failed" => JobStatus::Failed,
				"cancelled" => JobStatus::Cancelled,
				_ => continue,
			};

			// Parse progress from database
			let progress = if let Some(progress_data) = &j.progress_data {
				rmp_serde::from_slice::<Progress>(progress_data)
					.ok()
					.and_then(|p| p.as_percentage())
					.unwrap_or(0.0)
			} else {
				0.0
			};

			all_jobs.push(JobInfo {
				id,
				name: j.name,
				status,
				progress,
				started_at: j.started_at.unwrap_or(j.created_at),
				completed_at: j.completed_at,
				error_message: j.error_message,
				parent_job_id: j.parent_job_id.and_then(|s| s.parse::<Uuid>().ok()),
			});
		}

		Ok(all_jobs)
	}

	/// Get detailed information about a specific job
	pub async fn get_job_info(&self, id: Uuid) -> JobResult<Option<JobInfo>> {
		let job_id = JobId(id);

		if let Some(running_job) = self.running_jobs.read().await.get(&job_id) {
			let handle = &running_job.handle;
			let status = handle.status();

			// Get latest progress from memory
			let progress = if let Some(progress) = running_job.latest_progress.lock().await.as_ref()
			{
				progress.as_percentage().unwrap_or(0.0)
			} else {
				0.0
			};

			// For running jobs, we also need the job name from database
			let job_name = match database::jobs::Entity::find_by_id(id.to_string())
				.one(self.db.conn())
				.await?
			{
				Some(db_job) => db_job.name,
				None => format!("Job {}", id), // Fallback if not in DB
			};

			return Ok(Some(JobInfo {
				id,
				name: job_name,
				status,
				progress,
				started_at: chrono::Utc::now(), // TODO: Get actual start time from DB
				completed_at: None,             // Running jobs aren't completed yet
				error_message: None,            // TODO: Get from handle if failed
				parent_job_id: None,            // TODO: Get from DB if needed
			}));
		}

		let job = database::jobs::Entity::find_by_id(id.to_string())
			.one(self.db.conn())
			.await?;

		Ok(job.and_then(|j| {
			let id = j.id.parse::<Uuid>().ok()?;
			let status = match j.status.as_str() {
				"queued" => JobStatus::Queued,
				"running" => JobStatus::Running,
				"paused" => JobStatus::Paused,
				"completed" => JobStatus::Completed,
				"failed" => JobStatus::Failed,
				"cancelled" => JobStatus::Cancelled,
				_ => return None,
			};

			let progress = if let Some(progress_data) = &j.progress_data {
				rmp_serde::from_slice::<Progress>(progress_data)
					.ok()
					.and_then(|p| p.as_percentage())
					.unwrap_or(0.0)
			} else {
				0.0
			};

			Some(JobInfo {
				id,
				name: j.name,
				status,
				progress,
				started_at: j.started_at.unwrap_or(j.created_at),
				completed_at: j.completed_at,
				error_message: j.error_message,
				parent_job_id: j.parent_job_id.and_then(|s| s.parse::<Uuid>().ok()),
			})
		}))
	}

	/// Resume interrupted jobs from the last run
	async fn resume_interrupted_jobs(&self) -> JobResult<()> {
		info!("Checking for interrupted jobs to resume");

		use sea_orm::{ColumnTrait, QueryFilter};
		let interrupted = database::jobs::Entity::find()
			.filter(database::jobs::Column::Status.is_in([
				JobStatus::Running.to_string(),
				JobStatus::Paused.to_string(),
			]))
			.all(self.db.conn())
			.await?;

		for job_record in interrupted {
			if let Ok(job_id) = job_record.id.parse::<Uuid>().map(JobId) {
				info!("Resuming job {}: {}", job_id, job_record.name);

				// Deserialize job from binary data
				match REGISTRY.deserialize_job(&job_record.name, &job_record.state) {
					Ok(erased_job) => {
						// Create channels for the resumed job
						let (status_tx, status_rx) = watch::channel(JobStatus::Paused);
						let (progress_tx, progress_rx) = mpsc::unbounded_channel();
						let (broadcast_tx, broadcast_rx) = broadcast::channel(100);

						let latest_progress = Arc::new(Mutex::new(None));

						// Create progress forwarding task
						let broadcast_tx_clone = broadcast_tx.clone();
						let latest_progress_clone = latest_progress.clone();
						tokio::spawn(async move {
							let mut progress_rx: mpsc::UnboundedReceiver<Progress> = progress_rx;
							while let Some(progress) = progress_rx.recv().await {
								*latest_progress_clone.lock().await = Some(progress.clone());
								let _ = broadcast_tx_clone.send(progress);
							}
						});

						// Get library from context using stored library_id
						let library = self
							.context
							.library_manager
							.get_library(self.library_id)
							.await
							.ok_or_else(|| {
								JobError::invalid_state(&format!(
									"Library {} not found",
									self.library_id
								))
							})?;

						// Get services from context
						let networking = self.context.get_networking().await;
						let volume_manager = Some(self.context.volume_manager.clone());

						// Create executor using the erased job
						let executor = erased_job.create_executor(
							job_id,
							library,
							self.db.clone(),
							status_tx.clone(),
							progress_tx,
							broadcast_tx,
							Arc::new(DbCheckpointHandler {
								db: self.db.clone(),
							}),
							networking,
							volume_manager,
						);

						// Create handle
						let handle = JobHandle {
							id: job_id,
							task_handle: Arc::new(Mutex::new(None)),
							status_rx,
							progress_rx: broadcast_rx,
							output: Arc::new(Mutex::new(None)),
						};

						// Dispatch to task system
						match self
							.dispatcher
							.get_dispatcher()
							.dispatch_boxed(executor)
							.await
						{
							Ok(task_handle) => {
								// Track running job
								self.running_jobs.write().await.insert(
									job_id,
									RunningJob {
										handle: handle.clone(),
										task_handle,
										status_tx: status_tx.clone(),
										latest_progress,
									},
								);

								// Spawn a task to monitor resumed job completion and clean up
								let running_jobs = self.running_jobs.clone();
								let job_id_clone = job_id.clone();
								let event_bus = self.context.events.clone();
								let job_type_str = job_record.name.clone();
								tokio::spawn(async move {
									let mut status_rx = status_tx.subscribe();
									while status_rx.changed().await.is_ok() {
										let status = *status_rx.borrow();
										match status {
											JobStatus::Completed => {
												// Emit completion event
												event_bus.emit(Event::JobCompleted {
													job_id: job_id_clone.to_string(),
													job_type: job_type_str.clone(),
												});
												// Remove from running jobs
												running_jobs.write().await.remove(&job_id_clone);
												info!("Resumed job {} completed and removed from running jobs", job_id_clone);
												break;
											}
											JobStatus::Failed => {
												// Emit failure event
												event_bus.emit(Event::JobFailed {
													job_id: job_id_clone.to_string(),
													job_type: job_type_str.clone(),
													error: "Job failed".to_string(),
												});
												// Remove from running jobs
												running_jobs.write().await.remove(&job_id_clone);
												info!("Resumed job {} failed and removed from running jobs", job_id_clone);
												break;
											}
											JobStatus::Cancelled => {
												// Emit cancellation event
												event_bus.emit(Event::JobCancelled {
													job_id: job_id_clone.to_string(),
													job_type: job_type_str.clone(),
												});
												// Remove from running jobs
												running_jobs.write().await.remove(&job_id_clone);
												info!("Resumed job {} cancelled and removed from running jobs", job_id_clone);
												break;
											}
											_ => {} // Continue monitoring for other status changes
										}
									}
								});

								info!("Successfully resumed job {}: {}", job_id, job_record.name);
							}
							Err(e) => {
								error!("Failed to dispatch resumed job {}: {:?}", job_id, e);
							}
						}
					}
					Err(e) => {
						error!("Failed to create job {} for resumption: {}", job_id, e);
					}
				}
			}
		}

		Ok(())
	}

	/// Pause a running job
	pub async fn pause_job(&self, job_id: JobId) -> JobResult<()> {
		let mut running_jobs = self.running_jobs.write().await;

		if let Some(running_job) = running_jobs.get_mut(&job_id) {
			// Check if job is in a pausable state
			let current_status = running_job.handle.status();
			if current_status != JobStatus::Running {
				return Err(JobError::invalid_state(&format!(
					"Cannot pause job in {:?} state",
					current_status
				)));
			}

			// Update status to Paused
			running_job
				.status_tx
				.send(JobStatus::Paused)
				.map_err(|e| JobError::Other(format!("Failed to update status: {}", e).into()))?;

			// The task will check status and interrupt itself when it sees Paused status
			// This happens through the executor checking the status channel

			// Update database
			use sea_orm::{ActiveModelTrait, ActiveValue::Set};
			let mut job_model = database::jobs::ActiveModel {
				id: Set(job_id.to_string()),
				status: Set(JobStatus::Paused.to_string()),
				paused_at: Set(Some(Utc::now())),
				..Default::default()
			};
			job_model.update(self.db.conn()).await?;

			// Emit pause event
			self.context.events.emit(Event::JobPaused {
				job_id: job_id.to_string(),
			});

			info!("Job {} paused successfully", job_id);
			Ok(())
		} else {
			Err(JobError::NotFound(format!("Job {} not found", job_id)))
		}
	}

	/// Resume a paused job
	pub async fn resume_job(&self, job_id: JobId) -> JobResult<()> {
		// First check if job exists in running jobs
		let job_info = {
			let running_jobs = self.running_jobs.read().await;
			if let Some(running_job) = running_jobs.get(&job_id) {
				// Check if job is paused
				let current_status = running_job.handle.status();
				if current_status != JobStatus::Paused {
					return Err(JobError::invalid_state(&format!(
						"Cannot resume job in {:?} state",
						current_status
					)));
				}
				None // Job is already in memory, just needs status update
			} else {
				// Job might be in database but not in memory
				drop(running_jobs);

				// Load job from database
				let job_record = database::jobs::Entity::find_by_id(job_id.to_string())
					.one(self.db.conn())
					.await?
					.ok_or_else(|| JobError::NotFound(format!("Job {} not found", job_id)))?;

				// Check if job is paused
				if job_record.status != JobStatus::Paused.to_string() {
					return Err(JobError::invalid_state(&format!(
						"Cannot resume job in {} state",
						job_record.status
					)));
				}

				Some((job_record.name.clone(), job_record.state.clone()))
			}
		};

		// If job was not in memory, recreate and dispatch it
		if let Some((job_name, job_state)) = job_info {
			// Deserialize job from binary data
			let erased_job = REGISTRY.deserialize_job(&job_name, &job_state)?;

			// Update database status to Running
			use sea_orm::{ActiveModelTrait, ActiveValue::Set};
			let mut job_model = database::jobs::ActiveModel {
				id: Set(job_id.to_string()),
				status: Set(JobStatus::Running.to_string()),
				paused_at: Set(None),
				..Default::default()
			};
			job_model.update(self.db.conn()).await?;

			// Create channels
			let (status_tx, status_rx) = watch::channel(JobStatus::Running);
			let (progress_tx, progress_rx) = mpsc::unbounded_channel();
			let (broadcast_tx, broadcast_rx) = broadcast::channel(100);

			let latest_progress = Arc::new(Mutex::new(None));

			// Create progress forwarding task
			let broadcast_tx_clone = broadcast_tx.clone();
			let latest_progress_clone = latest_progress.clone();
			let event_bus = self.context.events.clone();
			let job_id_clone = job_id.clone();
			let job_type_str = job_name.clone();
			tokio::spawn(async move {
				let mut progress_rx: mpsc::UnboundedReceiver<Progress> = progress_rx;
				while let Some(progress) = progress_rx.recv().await {
					*latest_progress_clone.lock().await = Some(progress.clone());
					let _ = broadcast_tx_clone.send(progress.clone());

					// Emit progress event
					event_bus.emit(Event::JobProgress {
						job_id: job_id_clone.to_string(),
						job_type: job_type_str.to_string(),
						progress: progress.as_percentage().unwrap_or(0.0) as f64,
						message: Some(progress.to_string()),
						generic_progress: None,
					});
				}
			});

			// Get library from context
			let library = self
				.context
				.library_manager
				.get_library(self.library_id)
				.await
				.ok_or_else(|| {
					JobError::invalid_state(&format!("Library {} not found", self.library_id))
				})?;

			// Get services from context
			let networking = self.context.get_networking().await;
			let volume_manager = Some(self.context.volume_manager.clone());

			// Create executor
			let executor = erased_job.create_executor(
				job_id,
				library,
				self.db.clone(),
				status_tx.clone(),
				progress_tx,
				broadcast_tx,
				Arc::new(DbCheckpointHandler {
					db: self.db.clone(),
				}),
				networking,
				volume_manager,
			);

			// Create handle
			let handle = JobHandle {
				id: job_id,
				task_handle: Arc::new(Mutex::new(None)),
				status_rx,
				progress_rx: broadcast_rx,
				output: Arc::new(Mutex::new(None)),
			};

			// Dispatch to task system
			let task_handle = self
				.dispatcher
				.get_dispatcher()
				.dispatch_boxed(executor)
				.await
				.map_err(|e| JobError::task_system(format!("Failed to dispatch: {:?}", e)))?;

			// Track running job
			self.running_jobs.write().await.insert(
				job_id,
				RunningJob {
					handle: handle.clone(),
					task_handle,
					status_tx: status_tx.clone(),
					latest_progress,
				},
			);

			// Spawn cleanup monitor
			let running_jobs = self.running_jobs.clone();
			let job_id_clone = job_id.clone();
			let event_bus = self.context.events.clone();
			let job_type_str = job_name.clone();
			tokio::spawn(async move {
				let mut status_rx = status_tx.subscribe();
				while status_rx.changed().await.is_ok() {
					let status = *status_rx.borrow();
					match status {
						JobStatus::Completed => {
							event_bus.emit(Event::JobCompleted {
								job_id: job_id_clone.to_string(),
								job_type: job_type_str.clone(),
							});
							running_jobs.write().await.remove(&job_id_clone);
							info!("Resumed job {} completed", job_id_clone);
							break;
						}
						JobStatus::Failed => {
							event_bus.emit(Event::JobFailed {
								job_id: job_id_clone.to_string(),
								job_type: job_type_str.clone(),
								error: "Job failed".to_string(),
							});
							running_jobs.write().await.remove(&job_id_clone);
							info!("Resumed job {} failed", job_id_clone);
							break;
						}
						JobStatus::Cancelled => {
							event_bus.emit(Event::JobCancelled {
								job_id: job_id_clone.to_string(),
								job_type: job_type_str.clone(),
							});
							running_jobs.write().await.remove(&job_id_clone);
							info!("Resumed job {} cancelled", job_id_clone);
							break;
						}
						_ => {}
					}
				}
			});

			// Emit resume event
			self.context.events.emit(Event::JobResumed {
				job_id: job_id.to_string(),
			});

			info!("Job {} resumed from database", job_id);
		} else {
			// Job is already in memory, just update status
			let mut running_jobs = self.running_jobs.write().await;
			if let Some(running_job) = running_jobs.get_mut(&job_id) {
				// Update status to Running
				running_job
					.status_tx
					.send(JobStatus::Running)
					.map_err(|e| {
						JobError::Other(format!("Failed to update status: {}", e).into())
					})?;

				// Update database
				use sea_orm::{ActiveModelTrait, ActiveValue::Set};
				let mut job_model = database::jobs::ActiveModel {
					id: Set(job_id.to_string()),
					status: Set(JobStatus::Running.to_string()),
					paused_at: Set(None),
					..Default::default()
				};
				job_model.update(self.db.conn()).await?;

				// Emit resume event
				self.context.events.emit(Event::JobResumed {
					job_id: job_id.to_string(),
				});

				info!("Job {} resumed", job_id);
			}
		}

		Ok(())
	}

	/// Shutdown the job manager
	pub async fn shutdown(&self) -> JobResult<()> {
		info!("Shutting down job manager");

		// First, pause all running jobs
		let job_ids: Vec<JobId> = self.running_jobs.read().await.keys().copied().collect();

		info!("Pausing {} running jobs before shutdown", job_ids.len());
		for job_id in &job_ids {
			// Check if job is still running before pausing
			if let Some(running_job) = self.running_jobs.read().await.get(job_id) {
				let status = running_job.handle.status();
				if status == JobStatus::Running {
					info!("Pausing job {} for shutdown", job_id);
					if let Err(e) = self.pause_job(*job_id).await {
						warn!("Failed to pause job {} during shutdown: {}", job_id, e);
						// Continue with shutdown even if pause fails
					}
				}
			}
		}

		// Signal shutdown
		let _ = self.shutdown_tx.send(true);

		// Wait for all jobs to finish pausing
		let start_time = tokio::time::Instant::now();
		let timeout = std::time::Duration::from_secs(10);

		loop {
			let running_count = self.running_jobs.read().await.len();
			if running_count == 0 {
				info!("All jobs have stopped");
				break;
			}

			if start_time.elapsed() > timeout {
				warn!("Timeout waiting for {} jobs to stop", running_count);
				break;
			}

			tokio::time::sleep(std::time::Duration::from_millis(100)).await;
		}

		Ok(())
	}
}

/// Checkpoint handler that uses the job database
struct DbCheckpointHandler {
	db: Arc<JobDb>,
}

#[async_trait]
impl CheckpointHandler for DbCheckpointHandler {
	async fn save_checkpoint(&self, job_id: JobId, data: Option<Vec<u8>>) -> JobResult<()> {
		use database::checkpoint;

		let checkpoint = checkpoint::ActiveModel {
			job_id: Set(job_id.to_string()),
			checkpoint_data: Set(data.unwrap_or_default()),
			created_at: Set(Utc::now()),
		};

		// Insert or update
		match checkpoint.clone().insert(self.db.conn()).await {
			Ok(model) => model,
			Err(_) => checkpoint.update(self.db.conn()).await?,
		};

		Ok(())
	}

	async fn load_checkpoint(&self, job_id: JobId) -> JobResult<Option<Vec<u8>>> {
		use database::checkpoint;

		let checkpoint = checkpoint::Entity::find_by_id(job_id.to_string())
			.one(self.db.conn())
			.await?;

		Ok(checkpoint.map(|c| c.checkpoint_data))
	}

	async fn delete_checkpoint(&self, job_id: JobId) -> JobResult<()> {
		use database::checkpoint;

		checkpoint::Entity::delete_by_id(job_id.to_string())
			.exec(self.db.conn())
			.await?;

		Ok(())
	}
}

use uuid::Uuid;
