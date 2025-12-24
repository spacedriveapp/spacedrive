//! Job manager for scheduling and executing jobs
//! The job manager has its own database in the library directory, not the global library database.

use super::{
	context::CheckpointHandler,
	database::{self, JobDb},
	error::{JobError, JobResult},
	executor::JobExecutor,
	handle::JobHandle,
	output::JobOutput,
	progress::Progress,
	registry::REGISTRY,
	traits::{DynJob, Job, JobHandler},
	types::{ActionContextInfo, ErasedJob, JobId, JobInfo, JobPriority, JobStatus},
};
use crate::infra::action::context::ActionContext;
use crate::{
	context::CoreContext,
	infra::event::{Event, EventBus},
	library::Library,
};
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
	persistence_complete_rx: Option<tokio::sync::oneshot::Receiver<()>>,
}

impl JobManager {
	/// Create a new job manager
	pub async fn new(
		data_dir: PathBuf,
		context: Arc<CoreContext>,
		library_id: uuid::Uuid,
	) -> JobResult<Self> {
		// Initialize job database at library root
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

	/// Initialize job manager (without resuming jobs)
	pub async fn initialize(&self) -> JobResult<()> {
		info!("Job manager initialized for library {}", self.library_id);
		Ok(())
	}

	/// Resume interrupted jobs - should be called after library is fully loaded
	pub async fn resume_interrupted_jobs_after_load(&self) -> JobResult<()> {
		info!("Resuming interrupted jobs for library {}", self.library_id);
		if let Err(e) = self.resume_interrupted_jobs().await {
			error!("Failed to resume interrupted jobs: {}", e);
		}
		Ok(())
	}

	/// Dispatch a job for execution
	pub async fn dispatch<J>(&self, job: J) -> JobResult<JobHandle>
	where
		J: Job + JobHandler + DynJob,
	{
		self.dispatch_with_priority(job, JobPriority::NORMAL, None)
			.await
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
		// Try core job registry first
		if REGISTRY.has_job(job_name) {
			// Create job instance from core registry
			let erased_job = REGISTRY.create_job(job_name, params)?;
			return self
				.dispatch_erased_job(job_name, erased_job, priority, None)
				.await;
		}

		// Check if it's an extension job (contains colon)
		if job_name.contains(':') {
			// Try extension job registry
			if let Some(plugin_manager) = self.context.get_plugin_manager().await {
				let job_registry = plugin_manager.read().await.job_registry();

				if job_registry.has_job(job_name) {
					// Extract state JSON from params
					let state_json = serde_json::to_string(&params).map_err(|e| {
						JobError::serialization(format!("Failed to serialize params: {}", e))
					})?;

					// Create WasmJob from registry
					let wasm_job = job_registry
						.create_wasm_job(job_name, state_json)
						.map_err(|e| JobError::NotFound(e))?;

					// Box as ErasedJob and dispatch with the extension job name
					let erased_job = Box::new(wasm_job) as Box<dyn ErasedJob>;
					return self
						.dispatch_erased_job(job_name, erased_job, priority, None)
						.await;
				}
			}
		}

		// Job not found in either registry
		Err(JobError::NotFound(format!(
			"Job type '{}' not registered",
			job_name
		)))
	}

	/// Helper method to dispatch an erased job (extracted from dispatch_by_name)
	async fn dispatch_erased_job(
		&self,
		job_name: &str,
		erased_job: Box<dyn ErasedJob>,
		priority: JobPriority,
		action_context: Option<ActionContext>,
	) -> JobResult<JobHandle> {
		let job_id = JobId::new();
		let should_persist = erased_job.should_persist();

		info!(
			"Dispatching job {} ({}): {} [persist: {}]",
			job_id, job_name, job_name, should_persist
		);

		// Only persist to database if the job should be persisted
		if should_persist {
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
				action_context: Set(None),
				action_type: Set(None),
			};

			job_model.insert(self.db.conn()).await?;
		}

		// Create channels
		let (status_tx, status_rx) = watch::channel(JobStatus::Queued);
		let (progress_tx, progress_rx) = mpsc::unbounded_channel::<Progress>();
		let (broadcast_tx, broadcast_rx) = broadcast::channel::<Progress>(100);

		// Create storage for latest progress
		let latest_progress = Arc::new(Mutex::new(None));

		// Create progress forwarding task
		// For ephemeral jobs, skip database updates and event emission
		let broadcast_tx_clone = broadcast_tx.clone();
		let latest_progress_clone = latest_progress.clone();
		let event_bus = self.context.events.clone();
		let job_id_clone = job_id.clone();
		let job_type_str = job_name.to_string();
		let device_id = self
			.context
			.device_manager
			.device_id()
			.unwrap_or_else(|_| uuid::Uuid::nil());
		tokio::spawn(async move {
			let mut progress_rx: mpsc::UnboundedReceiver<Progress> = progress_rx;
			let mut last_emit = std::time::Instant::now();
			let throttle_duration = std::time::Duration::from_millis(100);

			while let Some(progress) = progress_rx.recv().await {
				*latest_progress_clone.lock().await = Some(progress.clone());
				let _ = broadcast_tx_clone.send(progress.clone());

				// Skip event updates for ephemeral jobs
				if !should_persist {
					continue;
				}

				// Throttle JobProgress events to prevent flooding the event bus
				let now = std::time::Instant::now();
				if now.duration_since(last_emit) < throttle_duration {
					continue;
				}
				last_emit = now;

				// Emit enhanced progress event
				use crate::infra::event::Event;

				// Extract generic progress data if available
				let generic_progress = match &progress {
					Progress::Structured(value) => {
						// Try to deserialize CopyProgress and convert to GenericProgress
						if let Ok(copy_progress) = serde_json::from_value::<
							crate::ops::files::copy::CopyProgress,
						>(value.clone())
						{
							use crate::infra::job::generic_progress::ToGenericProgress;
							Some(copy_progress.to_generic_progress())
						} else {
							None
						}
					}
					Progress::Generic(gp) => Some(gp.clone()),
					_ => None,
				};

				event_bus.emit(Event::JobProgress {
					job_id: job_id_clone.to_string(),
					job_type: job_type_str.to_string(),
					device_id,
					progress: progress.as_percentage().unwrap_or(0.0) as f64,
					message: Some(progress.to_string()),
					generic_progress,
				});
			}
		});

		// Get library from context using stored library_id
		let library = self
			.context
			.libraries()
			.await
			.get_library(self.library_id)
			.await
			.ok_or_else(|| {
				JobError::invalid_state(&format!("Library {} not found", self.library_id))
			})?;

		// Get services from context
		let networking = self.context.get_networking().await;
		let volume_manager = Some(self.context.volume_manager.clone());

		// Clone status_rx for cleanup task
		let status_rx_cleanup = status_rx.clone();

		// Create handle
		let handle = JobHandle {
			id: job_id,
			job_name: job_name.to_string(),
			task_handle: Arc::new(Mutex::new(None)),
			status_rx,
			progress_rx: broadcast_rx,
			output: Arc::new(Mutex::new(None)),
		};

		// Create persistence completion channel
		let (persistence_complete_tx, persistence_complete_rx) = tokio::sync::oneshot::channel();

		// Only enable job file logging for persistent jobs (or if explicitly configured)
		let job_logging_config = if should_persist
			|| self
				.context
				.job_logging_config
				.as_ref()
				.map(|c| c.log_ephemeral_jobs)
				.unwrap_or(false)
		{
			self.context.job_logging_config.clone()
		} else {
			None
		};

		// Create executor using the erased job
		let executor = erased_job.create_executor(
			job_id,
			job_name.to_string(),
			library.clone(),
			self.db.clone(),
			status_tx.clone(),
			progress_tx,
			broadcast_tx,
			Arc::new(DbCheckpointHandler {
				db: self.db.clone(),
			}),
			handle.output.clone(),
			networking,
			volume_manager,
			job_logging_config,
			Some(library.job_logs_dir()),
			Some(persistence_complete_tx),
		);

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
						persistence_complete_rx: Some(persistence_complete_rx),
					},
				);

				// Spawn a task to monitor job completion and clean up
				let running_jobs = self.running_jobs.clone();
				let job_id_clone = job_id.clone();
				let event_bus = self.context.events.clone();
				let job_type_str = job_name.to_string();
				let library_id_clone = self.library_id;
				let context = self.context.clone();
				let device_id = self
					.context
					.device_manager
					.device_id()
					.unwrap_or_else(|_| uuid::Uuid::nil());
				tokio::spawn(async move {
					info!("Started cleanup monitor for job {}", job_id_clone);
					let mut status_monitor = status_rx_cleanup;
					while status_monitor.changed().await.is_ok() {
						let status = *status_monitor.borrow();
						info!("Job {} status changed to: {:?}", job_id_clone, status);
						match status {
							JobStatus::Running => {
								// Emit event for all jobs
								event_bus.emit(Event::JobStarted {
									job_id: job_id_clone.to_string(),
									job_type: job_type_str.to_string(),
									device_id,
								});
								info!("Emitted JobStarted event for job {}", job_id_clone);
							}
							JobStatus::Completed => {
								// Emit completion event for all jobs
								if should_persist {
									// Get the final output from the handle before removing the job
									let output = {
										let jobs = running_jobs.read().await;
										if let Some(job) = jobs.get(&job_id_clone) {
											job.handle
												.output
												.lock()
												.await
												.clone()
												.unwrap_or(Ok(JobOutput::Success))
										} else {
											Ok(JobOutput::Success)
										}
									};

									// Emit completion event with the job's output
									event_bus.emit(Event::JobCompleted {
										job_id: job_id_clone.to_string(),
										job_type: job_type_str.to_string(),
										device_id,
										output: output.unwrap_or(JobOutput::Success),
									});

									// Trigger library statistics recalculation after job completion
									let library_id_for_stats = library_id_clone;
									let context_for_stats = context.clone();
									tokio::spawn(async move {
										if let Some(library) = context_for_stats
											.libraries()
											.await
											.get_library(library_id_for_stats)
											.await
										{
											if let Err(e) = library.recalculate_statistics().await {
												warn!(
													library_id = %library_id_for_stats,
													job_id = %job_id_clone,
													error = %e,
													"Failed to trigger library statistics recalculation after job completion"
												);
											} else {
												debug!(
													library_id = %library_id_for_stats,
													job_id = %job_id_clone,
													"Triggered library statistics recalculation after job completion"
												);
											}
										}
									});
								}

								// Remove from running jobs
								running_jobs.write().await.remove(&job_id_clone);
								info!(
									"Job {} completed and removed from running jobs",
									job_id_clone
								);
								break;
							}
							JobStatus::Failed => {
								// Emit event for all jobs
								if should_persist {
									event_bus.emit(Event::JobFailed {
										job_id: job_id_clone.to_string(),
										job_type: job_type_str.to_string(),
										device_id,
										error: "Job failed".to_string(),
									});
								}
								// Remove from running jobs
								running_jobs.write().await.remove(&job_id_clone);
								info!("Job {} failed and removed from running jobs", job_id_clone);
								break;
							}
							JobStatus::Cancelled => {
								// Emit event for all jobs
								if should_persist {
									event_bus.emit(Event::JobCancelled {
										job_id: job_id_clone.to_string(),
										job_type: job_type_str.to_string(),
										device_id,
									});
								}
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

	/// Dispatch a job with specific priority and optional action context
	pub async fn dispatch_with_priority<J>(
		&self,
		job: J,
		priority: JobPriority,
		action_context: Option<ActionContext>,
	) -> JobResult<JobHandle>
	where
		J: Job + JobHandler + DynJob,
	{
		let job_id = JobId::new();
		let should_persist = job.should_persist();

		if let Some(ref ctx) = action_context {
			info!(
				"Dispatching job {}: {} (from action: {}) [persist: {}]",
				job_id,
				J::NAME,
				ctx.action_type,
				should_persist
			);
		} else {
			info!(
				"Dispatching job {}: {} [persist: {}]",
				job_id,
				J::NAME,
				should_persist
			);
		}

		// Only persist to database if the job should be persisted
		if should_persist {
			// Serialize job state
			let state =
				rmp_serde::to_vec(&job).map_err(|e| JobError::serialization(format!("{}", e)))?;

			// Serialize action context if provided
			let serialized_action_context = if let Some(ref ctx) = action_context {
				Some(
					rmp_serde::to_vec(ctx)
						.map_err(|e| JobError::serialization(format!("{}", e)))?,
				)
			} else {
				None
			};

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
				action_context: Set(serialized_action_context),
				action_type: Set(action_context.as_ref().map(|ctx| ctx.action_type.clone())),
			};

			job_model.insert(self.db.conn()).await?;
		}

		// Create channels
		let (status_tx, status_rx) = watch::channel(JobStatus::Queued);
		let (progress_tx, progress_rx) = mpsc::unbounded_channel::<Progress>();
		let (broadcast_tx, broadcast_rx) = broadcast::channel::<Progress>(100);

		// Create storage for latest progress
		let latest_progress = Arc::new(Mutex::new(None));

		// Create progress forwarding task with batching and throttling
		// For ephemeral jobs, skip database updates and event emission
		let broadcast_tx_clone = broadcast_tx.clone();
		let latest_progress_clone = latest_progress.clone();
		let event_bus = self.context.events.clone();
		let job_id_clone = job_id.clone();
		let job_type_str = J::NAME;
		let job_db_clone = self.db.clone();
		let device_id = self
			.context
			.device_manager
			.device_id()
			.unwrap_or_else(|_| uuid::Uuid::nil());

		tokio::spawn(async move {
			let mut progress_rx: mpsc::UnboundedReceiver<Progress> = progress_rx;
			let mut last_db_update = std::time::Instant::now();
			let mut last_event_emit = std::time::Instant::now();
			const DB_UPDATE_INTERVAL: std::time::Duration = std::time::Duration::from_secs(2);
			const EVENT_EMIT_INTERVAL: std::time::Duration = std::time::Duration::from_millis(100);

			while let Some(progress) = progress_rx.recv().await {
				// Store latest progress
				*latest_progress_clone.lock().await = Some(progress.clone());

				// Forward progress from mpsc to broadcast
				// Ignore errors if no one is listening
				let _ = broadcast_tx_clone.send(progress.clone());

				// Skip database and event updates for ephemeral jobs
				if !should_persist {
					continue;
				}

				// Persist progress to database with throttling
				if last_db_update.elapsed() >= DB_UPDATE_INTERVAL {
					if let Err(e) = job_db_clone.update_progress(job_id_clone, &progress).await {
						debug!("Failed to persist job progress to database: {}", e);
					}
					last_db_update = std::time::Instant::now();
				}

				// Throttle event emission to prevent flooding
				if last_event_emit.elapsed() < EVENT_EMIT_INTERVAL {
					continue;
				}
				last_event_emit = std::time::Instant::now();

				// Emit enhanced progress event
				use crate::infra::event::Event;

				// Extract generic progress data if available
				let generic_progress = match &progress {
					Progress::Structured(value) => {
						// Try to deserialize CopyProgress and convert to GenericProgress
						if let Ok(copy_progress) = serde_json::from_value::<
							crate::ops::files::copy::CopyProgress,
						>(value.clone())
						{
							use crate::infra::job::generic_progress::ToGenericProgress;
							Some(copy_progress.to_generic_progress())
						} else {
							None
						}
					}
					Progress::Generic(gp) => Some(gp.clone()),
					_ => None,
				};

				event_bus.emit(Event::JobProgress {
					job_id: job_id_clone.to_string(),
					job_type: job_type_str.to_string(),
					device_id,
					progress: progress.as_percentage().unwrap_or(0.0) as f64,
					message: Some(progress.to_string()),
					generic_progress,
				});
			}

			// Final progress update when channel closes
			if should_persist {
				if let Some(final_progress) = &*latest_progress_clone.lock().await {
					if let Err(e) = job_db_clone
						.update_progress(job_id_clone, final_progress)
						.await
					{
						debug!("Failed to persist final job progress to database: {}", e);
					}
				}
			}
		});

		// Get library from context using stored library_id
		let library = self
			.context
			.libraries()
			.await
			.get_library(self.library_id)
			.await
			.ok_or_else(|| {
				JobError::invalid_state(&format!("Library {} not found", self.library_id))
			})?;

		// Get services from context
		let networking = self.context.get_networking().await;
		let volume_manager = Some(self.context.volume_manager.clone());

		// Clone status_rx for cleanup task
		let status_rx_cleanup = status_rx.clone();

		// Create handle
		let handle = JobHandle {
			id: job_id,
			job_name: J::NAME.to_string(),
			task_handle: Arc::new(Mutex::new(None)),
			status_rx,
			progress_rx: broadcast_rx,
			output: Arc::new(Mutex::new(None)),
		};

		// Create persistence completion channel
		let (persistence_complete_tx, persistence_complete_rx) = tokio::sync::oneshot::channel();

		// Only enable job file logging for persistent jobs (or if explicitly configured)
		let job_logging_config = if should_persist
			|| self
				.context
				.job_logging_config
				.as_ref()
				.map(|c| c.log_ephemeral_jobs)
				.unwrap_or(false)
		{
			self.context.job_logging_config.clone()
		} else {
			None
		};

		// Create executor
		let executor = JobExecutor::new(
			job,
			job_id,
			J::NAME.to_string(),
			library.clone(),
			self.db.clone(),
			status_tx.clone(),
			progress_tx,
			broadcast_tx,
			Arc::new(DbCheckpointHandler {
				db: self.db.clone(),
			}),
			handle.output.clone(),
			networking,
			volume_manager,
			job_logging_config,
			Some(library.job_logs_dir()),
			Some(persistence_complete_tx),
		);

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
						persistence_complete_rx: Some(persistence_complete_rx),
					},
				);

				// Spawn a task to monitor job completion and clean up
				let running_jobs = self.running_jobs.clone();
				let job_id_clone = job_id.clone();
				let event_bus = self.context.events.clone();
				let job_type_str = J::NAME;
				let library_id_clone = self.library_id;
				let context = self.context.clone();
				let device_id = self
					.context
					.device_manager
					.device_id()
					.unwrap_or_else(|_| uuid::Uuid::nil());
				tokio::spawn(async move {
					info!("Started cleanup monitor for job {}", job_id_clone);
					let mut status_monitor = status_rx_cleanup;
					while status_monitor.changed().await.is_ok() {
						let status = *status_monitor.borrow();
						info!("Job {} status changed to: {:?}", job_id_clone, status);
						match status {
							JobStatus::Running => {
								// Emit event for all jobs
								event_bus.emit(Event::JobStarted {
									job_id: job_id_clone.to_string(),
									job_type: job_type_str.to_string(),
									device_id,
								});
								info!("Emitted JobStarted event for job {}", job_id_clone);
							}
							JobStatus::Completed => {
								// Emit completion event for all jobs
								if should_persist {
									// Get the final output from the handle before removing the job
									let output = {
										let jobs = running_jobs.read().await;
										if let Some(job) = jobs.get(&job_id_clone) {
											job.handle
												.output
												.lock()
												.await
												.clone()
												.unwrap_or(Ok(JobOutput::Success))
										} else {
											Ok(JobOutput::Success)
										}
									};

									// Emit completion event with the job's output
									event_bus.emit(Event::JobCompleted {
										job_id: job_id_clone.to_string(),
										job_type: job_type_str.to_string(),
										device_id,
										output: output.unwrap_or(JobOutput::Success),
									});

									// Trigger library statistics recalculation after job completion
									let library_id_for_stats = library_id_clone;
									let context_for_stats = context.clone();
									tokio::spawn(async move {
										if let Some(library) = context_for_stats
											.libraries()
											.await
											.get_library(library_id_for_stats)
											.await
										{
											if let Err(e) = library.recalculate_statistics().await {
												warn!(
													library_id = %library_id_for_stats,
													job_id = %job_id_clone,
													error = %e,
													"Failed to trigger library statistics recalculation after job completion"
												);
											} else {
												debug!(
													library_id = %library_id_for_stats,
													job_id = %job_id_clone,
													"Triggered library statistics recalculation after job completion"
												);
											}
										}
									});
								}

								// Remove from running jobs
								running_jobs.write().await.remove(&job_id_clone);
								info!(
									"Job {} completed and removed from running jobs",
									job_id_clone
								);
								break;
							}
							JobStatus::Failed => {
								// Emit event for all jobs
								if should_persist {
									event_bus.emit(Event::JobFailed {
										job_id: job_id_clone.to_string(),
										job_type: job_type_str.to_string(),
										device_id,
										error: "Job failed".to_string(),
									});
								}
								// Remove from running jobs
								running_jobs.write().await.remove(&job_id_clone);
								info!("Job {} failed and removed from running jobs", job_id_clone);
								break;
							}
							JobStatus::Cancelled => {
								// Emit event for all jobs
								if should_persist {
									event_bus.emit(Event::JobCancelled {
										job_id: job_id_clone.to_string(),
										job_type: job_type_str.to_string(),
										device_id,
									});
								}
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

	/// List currently running jobs from memory (for live monitoring)
	pub async fn list_running_jobs(&self) -> Vec<JobInfo> {
		let device_id = self
			.context
			.device_manager
			.device_id()
			.unwrap_or_else(|_| uuid::Uuid::nil());
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
					device_id,
					status,
					progress: progress_percentage,
					created_at: chrono::Utc::now(), // Running jobs use current time as fallback
					started_at: Some(chrono::Utc::now()), // Running jobs have started
					completed_at: None,
					error_message: None,
					parent_job_id: None,
					action_type: None,
					action_context: None,
				};

				job_infos.push(job_info);
			}
		}

		job_infos
	}

	/// List all jobs with a specific status (unified query)
	pub async fn list_jobs(&self, status: Option<JobStatus>) -> JobResult<Vec<JobInfo>> {
		use sea_orm::QueryFilter;

		let device_id = self
			.context
			.device_manager
			.device_id()
			.unwrap_or_else(|_| uuid::Uuid::nil());

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

			// Get job data from database for complete info
			let (job_name, action_type, action_context) =
				match database::jobs::Entity::find_by_id(job_id.0.to_string())
					.one(self.db.conn())
					.await?
				{
					Some(db_job) => {
						let action_context = if let Some(context_data) = &db_job.action_context {
							match rmp_serde::from_slice::<
								crate::infra::action::context::ActionContext,
							>(context_data)
							{
								Ok(ctx) => Some(ActionContextInfo {
									action_type: ctx.action_type.clone(),
									initiated_at: ctx.initiated_at,
									initiated_by: ctx.initiated_by.clone(),
									action_input: ctx.action_input.into(),
									context: ctx.context.into(),
								}),
								Err(_) => None,
							}
						} else {
							None
						};
						(db_job.name, db_job.action_type, action_context)
					}
					None => (format!("Job {}", job_id.0), None, None),
				};

			all_jobs.push(JobInfo {
				id: job_id.0,
				name: job_name,
				device_id,
				status: current_status,
				progress: progress_percentage,
				created_at: chrono::Utc::now(), // Running jobs use current time as fallback
				started_at: Some(chrono::Utc::now()), // Running jobs have started
				completed_at: None,
				error_message: None,
				parent_job_id: None,
				action_type,
				action_context,
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

			// Parse action context from database
			let action_context = if let Some(context_data) = &j.action_context {
				match rmp_serde::from_slice::<crate::infra::action::context::ActionContext>(
					context_data,
				) {
					Ok(ctx) => Some(ActionContextInfo {
						action_type: ctx.action_type.clone(),
						initiated_at: ctx.initiated_at,
						initiated_by: ctx.initiated_by.clone(),
						action_input: ctx.action_input.into(),
						context: ctx.context.into(),
					}),
					Err(_) => None,
				}
			} else {
				None
			};

			all_jobs.push(JobInfo {
				id,
				name: j.name,
				device_id,
				status,
				progress,
				created_at: j.created_at,
				started_at: j.started_at,
				completed_at: j.completed_at,
				error_message: j.error_message,
				parent_job_id: j.parent_job_id.and_then(|s| s.parse::<Uuid>().ok()),
				action_type: j.action_type,
				action_context,
			});
		}

		Ok(all_jobs)
	}

	/// Get detailed information about a specific job
	pub async fn get_job_info(&self, id: Uuid) -> JobResult<Option<JobInfo>> {
		let device_id = self
			.context
			.device_manager
			.device_id()
			.unwrap_or_else(|_| uuid::Uuid::nil());
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
				device_id,
				status,
				progress,
				created_at: chrono::Utc::now(), // Running jobs use current time as fallback
				started_at: Some(chrono::Utc::now()), // Running jobs have started
				completed_at: None,             // Running jobs aren't completed yet
				error_message: None,            // TODO: Get from handle if failed
				parent_job_id: None,            // TODO: Get from DB if needed
				action_type: None,
				action_context: None,
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
				device_id,
				status,
				progress,
				created_at: j.created_at,
				started_at: j.started_at,
				completed_at: j.completed_at,
				error_message: j.error_message,
				parent_job_id: j.parent_job_id.and_then(|s| s.parse::<Uuid>().ok()),
				action_type: j.action_type,
				action_context: None, // TODO: Parse action context from j.action_context
			})
		}))
	}

	/// Resume interrupted jobs from the last run
	async fn resume_interrupted_jobs(&self) -> JobResult<()> {
		warn!(
			"DEBUG: resume_interrupted_jobs called for library {}",
			self.library_id
		);
		info!("Checking for interrupted jobs to resume");

		use sea_orm::{ColumnTrait, QueryFilter};
		let interrupted = database::jobs::Entity::find()
			.filter(database::jobs::Column::Status.is_in([
				JobStatus::Running.to_string(),
				JobStatus::Paused.to_string(),
			]))
			.all(self.db.conn())
			.await?;

		warn!(
			"DEBUG: Found {} interrupted jobs to resume",
			interrupted.len()
		);
		for job_record in interrupted {
			if let Ok(job_id) = job_record.id.parse::<Uuid>().map(JobId) {
				warn!(
					"DEBUG: Processing interrupted job {}: {} with status {}",
					job_id, job_record.name, job_record.status
				);
				info!("Resuming job {}: {}", job_id, job_record.name);

				// Deserialize job from binary data
				warn!(
					"DEBUG: Attempting to deserialize job {} of type {}",
					job_id, job_record.name
				);
				info!(
					"RESUME_STATE_LOAD: Job {} loading {} bytes of state from database",
					job_id,
					job_record.state.len()
				);
				match REGISTRY.deserialize_job(&job_record.name, &job_record.state) {
					Ok(erased_job) => {
						warn!("DEBUG: Successfully deserialized job {}", job_id);
						info!(
							"RESUME_STATE_LOAD: Job {} successfully deserialized {} bytes of state",
							job_id,
							job_record.state.len()
						);
						// Create channels for the resumed job
						let (status_tx, status_rx) = watch::channel(JobStatus::Paused);
						let (progress_tx, progress_rx) = mpsc::unbounded_channel::<Progress>();
						let (broadcast_tx, broadcast_rx) = broadcast::channel::<Progress>(100);

						let latest_progress = Arc::new(Mutex::new(None));

						// Create progress forwarding task with event bus emission
						let broadcast_tx_clone = broadcast_tx.clone();
						let latest_progress_clone = latest_progress.clone();
						let event_bus = self.context.events.clone();
						let job_id_clone = job_id;
						let job_type_str = job_record.name.clone();
						let job_db_clone = self.db.clone();
						let device_id = self
							.context
							.device_manager
							.device_id()
							.unwrap_or_else(|_| uuid::Uuid::nil());
						tokio::spawn(async move {
							let mut progress_rx: mpsc::UnboundedReceiver<Progress> = progress_rx;
							let mut last_db_update = std::time::Instant::now();
							let mut last_event_emit = std::time::Instant::now();
							const DB_UPDATE_INTERVAL: std::time::Duration =
								std::time::Duration::from_secs(2);
							const EVENT_EMIT_INTERVAL: std::time::Duration =
								std::time::Duration::from_millis(100);

							while let Some(progress) = progress_rx.recv().await {
								// Store latest progress
								*latest_progress_clone.lock().await = Some(progress.clone());

								// Forward progress from mpsc to broadcast
								let _ = broadcast_tx_clone.send(progress.clone());

								// Persist progress to database with throttling
								if last_db_update.elapsed() >= DB_UPDATE_INTERVAL {
									if let Err(e) =
										job_db_clone.update_progress(job_id_clone, &progress).await
									{
										debug!("Failed to persist job progress to database: {}", e);
									}
									last_db_update = std::time::Instant::now();
								}

								// Throttle event emission to prevent flooding
								if last_event_emit.elapsed() < EVENT_EMIT_INTERVAL {
									continue;
								}
								last_event_emit = std::time::Instant::now();

								// Emit enhanced progress event
								use crate::infra::event::Event;

								// Extract generic progress data if available
								let generic_progress = match &progress {
									Progress::Structured(value) => {
										// Try to deserialize CopyProgress and convert to GenericProgress
										if let Ok(copy_progress) = serde_json::from_value::<
											crate::ops::files::copy::CopyProgress,
										>(value.clone())
										{
											use crate::infra::job::generic_progress::ToGenericProgress;
											Some(copy_progress.to_generic_progress())
										} else {
											None
										}
									}
									Progress::Generic(gp) => Some(gp.clone()),
									_ => None,
								};

								event_bus.emit(Event::JobProgress {
									job_id: job_id_clone.to_string(),
									job_type: job_type_str.to_string(),
									device_id,
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
									debug!(
										"Failed to persist final job progress to database: {}",
										e
									);
								}
							}
						});

						// Get library from context using stored library_id
						let library = self
							.context
							.libraries()
							.await
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

						// Create handle
						let job_name = job_record.name.clone();
						let handle = JobHandle {
							id: job_id,
							job_name: job_name.clone(),
							task_handle: Arc::new(Mutex::new(None)),
							status_rx,
							progress_rx: broadcast_rx,
							output: Arc::new(Mutex::new(None)),
						};

						// Create persistence completion channel
						let (persistence_complete_tx, persistence_complete_rx) =
							tokio::sync::oneshot::channel();

						// Create executor using the erased job
						let executor = erased_job.create_executor(
							job_id,
							job_name,
							library,
							self.db.clone(),
							status_tx.clone(),
							progress_tx,
							broadcast_tx,
							Arc::new(DbCheckpointHandler {
								db: self.db.clone(),
							}),
							handle.output.clone(),
							networking,
							volume_manager,
							self.context.job_logging_config.clone(),
							self.context.job_logs_dir.clone(),
							Some(persistence_complete_tx),
						);

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
										persistence_complete_rx: Some(persistence_complete_rx),
									},
								);

								// Spawn a task to monitor resumed job completion and clean up
								let running_jobs = self.running_jobs.clone();
								let job_id_clone = job_id.clone();
								let event_bus = self.context.events.clone();
								let job_type_str = job_record.name.to_string();
								let library_id_clone = self.library_id;
								let context = self.context.clone();
								let device_id = self
									.context
									.device_manager
									.device_id()
									.unwrap_or_else(|_| uuid::Uuid::nil());
								tokio::spawn(async move {
									let mut status_rx = status_tx.subscribe();
									while status_rx.changed().await.is_ok() {
										let status = *status_rx.borrow();
										match status {
											JobStatus::Running => {
												// Emit JobStarted event for resumed jobs
												event_bus.emit(Event::JobStarted {
													job_id: job_id_clone.to_string(),
													job_type: job_type_str.to_string(),
													device_id,
												});
												info!(
													"Emitted JobStarted event for resumed job {}",
													job_id_clone
												);
											}
											JobStatus::Completed => {
												// Get the final output from the handle
												let output = {
													let jobs = running_jobs.read().await;
													if let Some(job) = jobs.get(&job_id_clone) {
														job.handle
															.output
															.lock()
															.await
															.clone()
															.unwrap_or(Ok(JobOutput::Success))
													} else {
														Ok(JobOutput::Success)
													}
												};

												// Emit completion event
												event_bus.emit(Event::JobCompleted {
													job_id: job_id_clone.to_string(),
													job_type: job_type_str.clone(),
													device_id,
													output: output.unwrap_or(JobOutput::Success),
												});

												// Trigger library statistics recalculation after job completion
												let library_id_for_stats = library_id_clone;
												let context_for_stats = context.clone();
												tokio::spawn(async move {
													if let Some(library) = context_for_stats
														.libraries()
														.await
														.get_library(library_id_for_stats)
														.await
													{
														if let Err(e) =
															library.recalculate_statistics().await
														{
															warn!(
																library_id = %library_id_for_stats,
																job_id = %job_id_clone,
																error = %e,
																"Failed to trigger library statistics recalculation after resumed job completion"
															);
														} else {
															debug!(
																library_id = %library_id_for_stats,
																job_id = %job_id_clone,
																"Triggered library statistics recalculation after resumed job completion"
															);
														}
													}
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
													device_id,
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
													device_id,
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

								// Update status to Running after successful dispatch
								warn!(
									"DEBUG: Attempting to update resumed job {} status to Running",
									job_id
								);
								if let Some(running_job) =
									self.running_jobs.read().await.get(&job_id)
								{
									if let Err(e) = running_job.status_tx.send(JobStatus::Running) {
										warn!("Failed to update resumed job status: {}", e);
									} else {
										warn!(
											"DEBUG: Successfully sent Running status to job {}",
											job_id
										);
									}
								} else {
									warn!("DEBUG: Job {} not found in running_jobs when trying to update status", job_id);
								}

								// Update database status
								warn!("DEBUG: Attempting to update database status for job {} to Running", job_id);
								use sea_orm::{ActiveModelTrait, ActiveValue::Set};
								let mut job_model = database::jobs::ActiveModel {
									id: Set(job_id.to_string()),
									status: Set(JobStatus::Running.to_string()),
									paused_at: Set(None),
									..Default::default()
								};
								if let Err(e) = job_model.update(self.db.conn()).await {
									warn!("Failed to update resumed job status in database: {}", e);
								} else {
									warn!("DEBUG: Successfully updated database status for job {} to Running", job_id);
								}

								info!("Successfully resumed job {}: {}", job_id, job_record.name);
							}
							Err(e) => {
								error!("Failed to dispatch resumed job {}: {:?}", job_id, e);
							}
						}
					}
					Err(e) => {
						warn!("DEBUG: Failed to deserialize job {}: {:?}", job_id, e);
						error!("Failed to create job {} for resumption: {}", job_id, e);
					}
				}
			}
		}

		Ok(())
	}

	/// Pause a running job
	pub async fn pause_job(&self, job_id: JobId) -> JobResult<()> {
		let device_id = self
			.context
			.device_manager
			.device_id()
			.unwrap_or_else(|_| uuid::Uuid::nil());
		let running_jobs = self.running_jobs.read().await;

		if let Some(running_job) = running_jobs.get(&job_id) {
			// Check if job is in a pausable state
			let current_status = running_job.handle.status();
			if current_status != JobStatus::Running {
				return Err(JobError::invalid_state(&format!(
					"Cannot pause job in {:?} state",
					current_status
				)));
			}

			// Update status to Paused FIRST (before triggering interrupt)
			running_job
				.status_tx
				.send(JobStatus::Paused)
				.map_err(|e| JobError::Other(format!("Failed to update status: {}", e).into()))?;

			// Trigger the actual task interruption through the task system
			if let Err(e) = running_job.task_handle.pause().await {
				warn!("Failed to pause task for job {}: {}", job_id, e);
				// Reset status back to Running if task pause failed
				let _ = running_job.status_tx.send(JobStatus::Running);
				return Err(JobError::Other(
					format!("Failed to pause task: {}", e).into(),
				));
			}

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
				device_id,
			});

			info!("Job {} paused successfully", job_id);
			Ok(())
		} else {
			Err(JobError::NotFound(format!("Job {} not found", job_id)))
		}
	}

	/// Cancel a running job
	pub async fn cancel_job(&self, job_id: JobId) -> JobResult<()> {
		let mut running_jobs = self.running_jobs.write().await;

		if let Some(running_job) = running_jobs.get_mut(&job_id) {
			// Check if job is in a cancellable state
			let current_status = running_job.handle.status();
			if current_status.is_terminal() {
				return Err(JobError::invalid_state(&format!(
					"Cannot cancel job in {:?} state",
					current_status
				)));
			}

			// Cancel the task - this will cause the executor to handle cancellation
			if let Err(e) = running_job.task_handle.cancel().await {
				warn!("Failed to send cancel signal to job {}: {}", job_id, e);
			}

			info!("Job {} cancellation requested", job_id);
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
			info!(
				"RESUME_STATE_LOAD: Job {} loading {} bytes of state from database (manual resume)",
				job_id,
				job_state.len()
			);
			let erased_job = REGISTRY.deserialize_job(&job_name, &job_state)?;
			info!("RESUME_STATE_LOAD: Job {} successfully deserialized {} bytes of state (manual resume)",
				job_id, job_state.len());

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
			let (progress_tx, progress_rx) = mpsc::unbounded_channel::<Progress>();
			let (broadcast_tx, broadcast_rx) = broadcast::channel::<Progress>(100);

			let latest_progress = Arc::new(Mutex::new(None));

			// Create progress forwarding task
			let broadcast_tx_clone = broadcast_tx.clone();
			let latest_progress_clone = latest_progress.clone();
			let event_bus = self.context.events.clone();
			let job_id_clone = job_id.clone();
			let job_type_str = job_name.clone();
			let device_id = self
				.context
				.device_manager
				.device_id()
				.unwrap_or_else(|_| uuid::Uuid::nil());
			tokio::spawn(async move {
				let mut progress_rx: mpsc::UnboundedReceiver<Progress> = progress_rx;
				let mut last_emit = std::time::Instant::now();
				let throttle_duration = std::time::Duration::from_millis(100);

				while let Some(progress) = progress_rx.recv().await {
					*latest_progress_clone.lock().await = Some(progress.clone());
					let _ = broadcast_tx_clone.send(progress.clone());

					// Throttle JobProgress events to prevent flooding the event bus
					let now = std::time::Instant::now();
					if now.duration_since(last_emit) < throttle_duration {
						continue;
					}
					last_emit = now;

					// Emit progress event
					event_bus.emit(Event::JobProgress {
						job_id: job_id_clone.to_string(),
						job_type: job_type_str.to_string(),
						device_id,
						progress: progress.as_percentage().unwrap_or(0.0) as f64,
						message: Some(progress.to_string()),
						generic_progress: None,
					});
				}
			});

			// Get library from context
			let library = self
				.context
				.libraries()
				.await
				.get_library(self.library_id)
				.await
				.ok_or_else(|| {
					JobError::invalid_state(&format!("Library {} not found", self.library_id))
				})?;

			// Get services from context
			let networking = self.context.get_networking().await;
			let volume_manager = Some(self.context.volume_manager.clone());

			// Create handle
			let handle = JobHandle {
				id: job_id,
				job_name: job_name.clone(),
				task_handle: Arc::new(Mutex::new(None)),
				status_rx,
				progress_rx: broadcast_rx,
				output: Arc::new(Mutex::new(None)),
			};

			// Create persistence completion channel
			let (persistence_complete_tx, persistence_complete_rx) =
				tokio::sync::oneshot::channel();

			// Create executor
			let executor = erased_job.create_executor(
				job_id,
				job_name.clone(),
				library,
				self.db.clone(),
				status_tx.clone(),
				progress_tx,
				broadcast_tx,
				Arc::new(DbCheckpointHandler {
					db: self.db.clone(),
				}),
				handle.output.clone(),
				networking,
				volume_manager,
				self.context.job_logging_config.clone(),
				self.context.job_logs_dir.clone(),
				Some(persistence_complete_tx),
			);

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
					persistence_complete_rx: Some(persistence_complete_rx),
				},
			);

			// Spawn cleanup monitor
			let running_jobs = self.running_jobs.clone();
			let job_id_clone = job_id.clone();
			let event_bus = self.context.events.clone();
			let job_type_str = job_name.clone();
			let library_id_clone = self.library_id;
			let context = self.context.clone();
			let device_id = self
				.context
				.device_manager
				.device_id()
				.unwrap_or_else(|_| uuid::Uuid::nil());
			tokio::spawn(async move {
				let mut status_rx = status_tx.subscribe();
				while status_rx.changed().await.is_ok() {
					let status = *status_rx.borrow();
					match status {
						JobStatus::Completed => {
							let output = {
								let jobs = running_jobs.read().await;
								if let Some(job) = jobs.get(&job_id_clone) {
									job.handle
										.output
										.lock()
										.await
										.clone()
										.unwrap_or(Ok(JobOutput::Success))
								} else {
									Ok(JobOutput::Success)
								}
							};
							event_bus.emit(Event::JobCompleted {
								job_id: job_id_clone.to_string(),
								job_type: job_type_str.clone(),
								device_id,
								output: output.unwrap_or(JobOutput::Success),
							});

							// Trigger library statistics recalculation after job completion
							let library_id_for_stats = library_id_clone;
							let context_for_stats = context.clone();
							tokio::spawn(async move {
								if let Some(library) = context_for_stats
									.libraries()
									.await
									.get_library(library_id_for_stats)
									.await
								{
									if let Err(e) = library.recalculate_statistics().await {
										warn!(
											library_id = %library_id_for_stats,
											job_id = %job_id_clone,
											error = %e,
											"Failed to trigger library statistics recalculation after resumed job completion"
										);
									} else {
										debug!(
											library_id = %library_id_for_stats,
											job_id = %job_id_clone,
											"Triggered library statistics recalculation after resumed job completion"
										);
									}
								}
							});

							running_jobs.write().await.remove(&job_id_clone);
							info!("Resumed job {} completed", job_id_clone);
							break;
						}
						JobStatus::Failed => {
							event_bus.emit(Event::JobFailed {
								job_id: job_id_clone.to_string(),
								job_type: job_type_str.clone(),
								device_id,
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
								device_id,
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
			let device_id = self
				.context
				.device_manager
				.device_id()
				.unwrap_or_else(|_| uuid::Uuid::nil());
			self.context.events.emit(Event::JobResumed {
				job_id: job_id.to_string(),
				device_id,
			});

			info!("Job {} resumed from database", job_id);
		} else {
			// Job is already in memory, just update status
			let device_id = self
				.context
				.device_manager
				.device_id()
				.unwrap_or_else(|_| uuid::Uuid::nil());
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
					device_id,
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
		let timeout = std::time::Duration::from_secs(30); // Increased timeout for large jobs
		let mut last_logged_count = 0;

		loop {
			let running_jobs = self.running_jobs.read().await;
			let total_count = running_jobs.len();

			// Count jobs that are actually still running (not paused)
			let still_running_count = running_jobs
				.values()
				.filter(|job| {
					let status = job.handle.status();
					status == JobStatus::Running
				})
				.count();

			if still_running_count == 0 {
				info!(
					"All jobs have been paused or stopped (total jobs: {}, still running: {})",
					total_count, still_running_count
				);
				break;
			}

			// Log progress every 5 seconds or when count changes
			if still_running_count != last_logged_count || start_time.elapsed().as_secs() % 5 == 0 {
				info!("Waiting for {} jobs to pause... ({:.1}s elapsed) (total jobs: {}, still running: {})",
					still_running_count, start_time.elapsed().as_secs_f32(), total_count, still_running_count);
				last_logged_count = still_running_count;
			}

			if start_time.elapsed() > timeout {
				warn!(
					"Timeout waiting for {} jobs to stop after {}s - forcing shutdown",
					still_running_count,
					timeout.as_secs()
				);

				// Log which jobs are still running
				for (job_id, running_job) in running_jobs.iter() {
					let status = running_job.handle.status();
					if status == JobStatus::Running {
						warn!("Job {} still running with status: {:?}", job_id, status);
					} else {
						info!(
							"Job {} has status: {:?} (not blocking shutdown)",
							job_id, status
						);
					}
				}
				break;
			}

			drop(running_jobs); // Release the lock before sleeping
			tokio::time::sleep(std::time::Duration::from_millis(500)).await;
		}

		// Wait for all paused jobs to complete state persistence
		info!("Waiting for job state persistence to complete...");
		let persistence_start_time = tokio::time::Instant::now();
		let persistence_timeout = std::time::Duration::from_secs(10); // Shorter timeout for persistence

		// Collect all persistence receivers
		let mut persistence_receivers = Vec::new();
		{
			let mut running_jobs = self.running_jobs.write().await;
			for (job_id, running_job) in running_jobs.iter_mut() {
				if let Some(rx) = running_job.persistence_complete_rx.take() {
					persistence_receivers.push((*job_id, rx));
				}
			}
		}

		info!(
			"Waiting for {} jobs to complete state persistence",
			persistence_receivers.len()
		);

		// Wait for all persistence operations to complete
		for (job_id, rx) in persistence_receivers {
			tokio::select! {
				result = rx => {
					match result {
						Ok(()) => {
							info!("Job {} completed state persistence", job_id);
						}
						Err(_) => {
							warn!("Job {} persistence channel closed without signal", job_id);
						}
					}
				}
				_ = tokio::time::sleep(persistence_timeout) => {
					warn!("Timeout waiting for job {} state persistence after {}s",
						job_id, persistence_timeout.as_secs());
					break;
				}
			}
		}

		let persistence_elapsed = persistence_start_time.elapsed();
		info!(
			"State persistence completed in {:.2}s",
			persistence_elapsed.as_secs_f32()
		);

		// Close database connection properly
		info!("Closing job database connection");

		// First, checkpoint the WAL file to merge it back into the main database
		use sea_orm::{ConnectionTrait, Statement};
		if let Err(e) = self
			.db
			.conn()
			.execute(Statement::from_string(
				sea_orm::DatabaseBackend::Sqlite,
				"PRAGMA wal_checkpoint(TRUNCATE)",
			))
			.await
		{
			warn!("Failed to checkpoint job database WAL file: {}", e);
		} else {
			info!("Job database WAL file checkpointed successfully");
		}

		if let Err(e) = self.db.conn().clone().close().await {
			warn!("Failed to close job database connection: {}", e);
		} else {
			info!("Job database connection closed successfully");
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
