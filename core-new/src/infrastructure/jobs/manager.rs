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
	traits::{Job, JobHandler},
	types::{JobId, JobInfo, JobPriority, JobStatus},
};
use crate::{context::CoreContext, infrastructure::events::{Event, EventBus}, library::Library};
use async_trait::async_trait;
use chrono::Utc;
use sd_task_system::{TaskDispatcher, TaskHandle, TaskSystem};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, DatabaseConnection, EntityTrait};
use std::{collections::HashMap, path::PathBuf, sync::Arc};
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
		println!(
			"🔍 JOB_DEBUG: Successfully inserted job {} into database",
			job_id
		);

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
						if let Ok(copy_progress) = serde_json::from_value::<crate::operations::files::copy::CopyProgress>(value.clone()) {
							use crate::infrastructure::jobs::generic_progress::ToGenericProgress;
							Some(serde_json::to_value(copy_progress.to_generic_progress()).ok())
						} else {
							None
						}
					}
					Progress::Generic(gp) => Some(serde_json::to_value(gp).ok()),
					_ => None,
				}.flatten();
				
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
								info!("Job {} completed and removed from running jobs", job_id_clone);
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
								info!("Job {} cancelled and removed from running jobs", job_id_clone);
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
		println!(
			"🔍 JOB_DEBUG: Successfully inserted job {} into database",
			job_id
		);

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
						if let Ok(copy_progress) = serde_json::from_value::<crate::operations::files::copy::CopyProgress>(value.clone()) {
							use crate::infrastructure::jobs::generic_progress::ToGenericProgress;
							Some(serde_json::to_value(copy_progress.to_generic_progress()).ok())
						} else {
							None
						}
					}
					Progress::Generic(gp) => Some(serde_json::to_value(gp).ok()),
					_ => None,
				}.flatten();
				
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
				if let Err(e) = job_db_clone.update_progress(job_id_clone, final_progress).await {
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
								info!("Job {} completed and removed from running jobs", job_id_clone);
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
								info!("Job {} cancelled and removed from running jobs", job_id_clone);
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

	/// List currently running jobs from memory (for live monitoring)
	pub async fn list_running_jobs(&self) -> Vec<JobInfo> {
		let running_jobs = self.running_jobs.read().await;
		let mut job_infos = Vec::new();

		info!(
			"list_running_jobs: Found {} jobs in running_jobs map",
			running_jobs.len()
		);
		
		// Debug: log all jobs in the map
		for (id, _) in running_jobs.iter() {
			debug!("Job {} is in running_jobs map", id);
		}

		for (job_id, running_job) in running_jobs.iter() {
			let handle = &running_job.handle;
			let status = handle.status();

			info!("Job {}: status = {:?}", job_id, status);

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
				info!(
					"Added active job {} to result with progress {:.1}%",
					job_id,
					progress_percentage * 100.0
				);
			} else {
				info!(
					"Skipping job {} with status {:?} (not active)",
					job_id, status
				);
			}
		}

		info!("Returning {} active jobs", job_infos.len());
		job_infos
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
			let progress_percentage = if let Some(progress) = running_job.latest_progress.lock().await.as_ref() {
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
				"Queued" => JobStatus::Queued,
				"Running" => JobStatus::Running,
				"Paused" => JobStatus::Paused,
				"Completed" => JobStatus::Completed,
				"Failed" => JobStatus::Failed,
				"Cancelled" => JobStatus::Cancelled,
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

		// First check if job is running in memory (for live status and progress)
		println!(
			"🔍 JOB_DEBUG: Checking for job {} in running jobs memory",
			id
		);
		if let Some(running_job) = self.running_jobs.read().await.get(&job_id) {
			println!("🔍 JOB_DEBUG: Found job {} in memory with live status", id);
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

		// Job not in memory, check database for completed/failed jobs
		println!(
			"🔍 JOB_DEBUG: Job {} not in memory, looking up in database",
			id
		);
		let job = database::jobs::Entity::find_by_id(id.to_string())
			.one(self.db.conn())
			.await?;

		if job.is_some() {
			println!("🔍 JOB_DEBUG: Found job {} in database", id);
		} else {
			println!("⚠️ JOB_DEBUG: Job {} NOT found in database", id);
		}

		Ok(job.and_then(|j| {
			println!(
				"🔍 JOB_DEBUG: Converting database job - status: {}, name: {}",
				j.status, j.name
			);
			let id = j.id.parse::<Uuid>().ok()?;
			let status = match j.status.as_str() {
				"Queued" => JobStatus::Queued,
				"Running" => JobStatus::Running,
				"Paused" => JobStatus::Paused,
				"Completed" => JobStatus::Completed,
				"Failed" => JobStatus::Failed,
				"Cancelled" => JobStatus::Cancelled,
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

	/// Shutdown the job manager
	pub async fn shutdown(&self) -> JobResult<()> {
		info!("Shutting down job manager");

		// Signal shutdown
		let _ = self.shutdown_tx.send(true);

		// Wait for all jobs to complete or pause
		let job_ids: Vec<JobId> = self.running_jobs.read().await.keys().copied().collect();
		for id in job_ids {
			debug!("Waiting for job {} to stop", id);
			// The task system will handle graceful shutdown
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
