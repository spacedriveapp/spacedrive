//! Job executor that wraps jobs for task system integration

use super::{
	context::{CheckpointHandler, JobContext},
	database::{self, JobDb},
	error::{JobError, JobResult},
	handle::JobHandle,
	output::JobOutput,
	progress::Progress,
	registry::REGISTRY,
	traits::{DynJob, Job, JobHandler},
	types::{ErasedJob, JobId, JobMetrics, JobStatus},
};
use crate::{config::JobLoggingConfig, library::Library};
use async_trait::async_trait;
use sd_task_system::{ExecStatus, Interrupter, Task, TaskId};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::{broadcast, mpsc, watch, Mutex};
use tracing::{debug, error, info, span, warn, Level};

/// Executor that wraps a job for task system execution
pub struct JobExecutor<J: JobHandler> {
	job: J,
	state: JobExecutorState,
}

pub struct JobExecutorState {
	pub job_id: JobId,
	pub library: Arc<Library>,
	pub job_db: Arc<JobDb>,
	pub status_tx: watch::Sender<super::types::JobStatus>,
	pub progress_tx: mpsc::UnboundedSender<Progress>,
	pub broadcast_tx: broadcast::Sender<Progress>,
	pub checkpoint_handler: Arc<dyn CheckpointHandler>,
	pub metrics: JobMetrics,
	pub output: Arc<Mutex<Option<JobResult<JobOutput>>>>,
	pub networking: Option<Arc<crate::service::network::NetworkingService>>,
	pub volume_manager: Option<Arc<crate::volume::VolumeManager>>,
	pub latest_progress: Arc<Mutex<Option<Progress>>>,
	pub job_logging_config: Option<JobLoggingConfig>,
	pub job_logs_dir: Option<PathBuf>,
	pub file_logger: Option<Arc<super::logger::FileJobLogger>>,
}

impl<J: JobHandler> JobExecutor<J> {
	pub fn new(
		job: J,
		job_id: JobId,
		library: Arc<Library>,
		job_db: Arc<JobDb>,
		status_tx: watch::Sender<super::types::JobStatus>,
		progress_tx: mpsc::UnboundedSender<Progress>,
		broadcast_tx: broadcast::Sender<Progress>,
		checkpoint_handler: Arc<dyn CheckpointHandler>,
		output_handle: Arc<Mutex<Option<JobResult<JobOutput>>>>,
		networking: Option<Arc<crate::service::network::NetworkingService>>,
		volume_manager: Option<Arc<crate::volume::VolumeManager>>,
		job_logging_config: Option<JobLoggingConfig>,
		job_logs_dir: Option<PathBuf>,
	) -> Self {
		// Create file logger if job logging is enabled
		let file_logger = if let (Some(config), Some(logs_dir)) = (&job_logging_config, &job_logs_dir) {
			let log_file = logs_dir.join(format!("{}.log", job_id));
			match super::logger::FileJobLogger::new(job_id, log_file, config.clone()) {
				Ok(logger) => {
					let _ = logger.log("INFO", &format!("Job {} ({}) starting", job_id, J::NAME));
					Some(Arc::new(logger))
				}
				Err(e) => {
					error!("Failed to create job logger: {}", e);
					None
				}
			}
		} else {
			None
		};

		Self {
			job,
			state: JobExecutorState {
				job_id,
				library,
				job_db,
				status_tx,
				progress_tx,
				broadcast_tx,
				checkpoint_handler,
				metrics: Default::default(),
				output: output_handle,
				networking,
				volume_manager,
				latest_progress: Arc::new(Mutex::new(None)),
				job_logging_config,
				job_logs_dir,
				file_logger,
			},
		}
	}

	/// Update job status in the database
	async fn update_job_status_in_db(&self, status: super::types::JobStatus) -> JobResult<()> {
		use super::database;
		use chrono::Utc;
		use sea_orm::{ActiveModelTrait, ActiveValue::Set};

		let mut job = database::jobs::ActiveModel {
			id: Set(self.state.job_id.to_string()),
			status: Set(status.to_string()),
			..Default::default()
		};

		// Update timestamps based on status
		match status {
			super::types::JobStatus::Running => {
				job.started_at = Set(Some(Utc::now()));
			}
			super::types::JobStatus::Paused => {
				job.paused_at = Set(Some(Utc::now()));
			}
			super::types::JobStatus::Completed
			| super::types::JobStatus::Failed
			| super::types::JobStatus::Cancelled => {
				job.completed_at = Set(Some(Utc::now()));
			}
			_ => {}
		}

		job.update(self.state.job_db.conn()).await?;
		Ok(())
	}
}

#[async_trait]
impl<J: JobHandler> Task<JobError> for JobExecutor<J> {
	fn id(&self) -> TaskId {
		TaskId::from(self.state.job_id.0)
	}

	fn with_priority(&self) -> bool {
		// TODO: Get from job priority
		false
	}

	async fn run(&mut self, interrupter: &Interrupter) -> Result<ExecStatus, JobError> {
		// Log job start
		if let Some(logger) = &self.state.file_logger {
			let _ = logger.log("INFO", &format!("Starting job {}: {}", self.state.job_id, J::NAME));
		}

		let result = self.run_inner(interrupter).await;

		// Log job completion
		if let Some(logger) = &self.state.file_logger {
			match &result {
				Ok(ExecStatus::Done(_)) => {
					let _ = logger.log("INFO", &format!("Job {} completed successfully", self.state.job_id));
				}
				Ok(ExecStatus::Canceled) => {
					let _ = logger.log("INFO", &format!("Job {} was cancelled", self.state.job_id));
				}
				Ok(ExecStatus::Paused) => {
					let _ = logger.log("INFO", &format!("Job {} was paused", self.state.job_id));
				}
				Err(e) => {
					let _ = logger.log("ERROR", &format!("Job {} failed: {}", self.state.job_id, e));
				}
			}
		}

		result
	}
}

impl<J: JobHandler> JobExecutor<J> {
	async fn run_inner(&mut self, interrupter: &Interrupter) -> Result<ExecStatus, JobError> {
		info!("Starting job {}: {}", self.state.job_id, J::NAME);

		// Update status to running
		warn!("DEBUG: JobExecutor setting status to Running for job {}", self.state.job_id);
		let _ = self.state.status_tx.send(super::types::JobStatus::Running);

		// Also persist status to database
		warn!("DEBUG: JobExecutor updating database status to Running for job {}", self.state.job_id);
		if let Err(e) = self
			.update_job_status_in_db(super::types::JobStatus::Running)
			.await
		{
			error!("Failed to update job status in database: {}", e);
		} else {
			warn!("DEBUG: JobExecutor successfully updated database status to Running for job {}", self.state.job_id);
		}

		// Create job context
		let ctx = JobContext {
			id: self.state.job_id,
			library: self.state.library.clone(),
			interrupter: interrupter,
			progress_tx: self.state.progress_tx.clone(),
			metrics: Arc::new(Mutex::new(self.state.metrics.clone())),
			checkpoint_handler: self.state.checkpoint_handler.clone(),
			child_handles: Arc::new(Mutex::new(Vec::new())),
			networking: self.state.networking.clone(),
			volume_manager: self.state.volume_manager.clone(),
			file_logger: self.state.file_logger.clone(),
		};

		// Progress forwarding is handled by JobManager

		// Check if we're resuming by checking if the job has existing state
		// This is a heuristic - if the job implements resumable logic, it should have state
		let is_resuming = self.job.is_resuming();
		warn!("DEBUG: Job {} is_resuming: {}", self.state.job_id, is_resuming);

		if is_resuming {
			warn!("DEBUG: Calling on_resume for job {}", self.state.job_id);
			if let Err(e) = self.job.on_resume(&ctx).await {
				error!("Job {} on_resume failed: {}", self.state.job_id, e);
				return Err(e);
			}
		}

		debug!("Starting job {}", self.state.job_id);

		// Store metrics reference for later update
		let metrics_ref = ctx.metrics.clone();

		// Run the job
		let result = self.job.run(ctx).await.map(|o| o.into());

		// Store the final result in the handle for the manager to retrieve
		*self.state.output.lock().await = Some(result.clone());

		match result {
			Ok(ref output) => {

				// Update metrics
				self.state.metrics = metrics_ref.lock().await.clone();

				// Update status with final progress
				info!("Job {} sending Completed status", self.state.job_id);
				if let Err(e) = self.state.status_tx.send(JobStatus::Completed) {
					error!(
						"Failed to send Completed status for job {}: {:?}",
						self.state.job_id, e
					);
				}

				// Persist final status and progress to database atomically
				let final_progress = if let Some(Ok(ref output)) = *self.state.output.lock().await {
					output.as_progress()
				} else {
					Some(Progress::percentage(1.0))
				};

				if let Err(e) = self
					.state
					.job_db
					.update_status_and_progress(
						self.state.job_id,
						JobStatus::Completed,
						final_progress.as_ref(),
						None,
					)
					.await
				{
					error!("Failed to update job completion status in database: {}", e);
				}

				info!(
					"Job {} completed successfully - status sent and DB updated",
					self.state.job_id
				);
				Ok(ExecStatus::Done(sd_task_system::TaskOutput::Empty))
			}
			Err(ref e) => {
				if e.is_interrupted() {
					debug!("Job {} interrupted", self.state.job_id);

					// Check current status to determine if this is a pause or cancel
					let current_status = *self.state.status_tx.borrow();

					if current_status == JobStatus::Paused {
						// Job was paused, don't update status (already set by pause_job)
						debug!("Job {} paused", self.state.job_id);

						// Save job state for resume
						use sea_orm::{ActiveModelTrait, ActiveValue::Set};
						let job_state = rmp_serde::to_vec(&self.job)
							.map_err(|e| JobError::serialization(format!("{}", e)))?;

						let mut job_model = super::database::jobs::ActiveModel {
							id: Set(self.state.job_id.to_string()),
							state: Set(job_state),
							..Default::default()
						};

						if let Err(e) = job_model.update(self.state.job_db.conn()).await {
							error!("Failed to save paused job state: {}", e);
						}

						Ok(ExecStatus::Canceled)
					} else {
						// Job was cancelled
						let _ = self.state.status_tx.send(JobStatus::Cancelled);

						// Persist cancellation status with latest progress
						let latest_progress = self.state.latest_progress.lock().await.clone();
						if let Err(e) = self
							.state
							.job_db
							.update_status_and_progress(
								self.state.job_id,
								JobStatus::Cancelled,
								latest_progress.as_ref(),
								None,
							)
							.await
						{
							error!(
								"Failed to update job cancellation status in database: {}",
								e
							);
						}

						Ok(ExecStatus::Canceled)
					}
				} else {
					error!("Job {} failed: {}", self.state.job_id, e);

					// Update status with current progress
					let _ = self.state.status_tx.send(JobStatus::Failed);

					// Persist failure status with latest progress and error message
					let latest_progress = self.state.latest_progress.lock().await.clone();
					if let Err(e_db) = self
						.state
						.job_db
						.update_status_and_progress(
							self.state.job_id,
							JobStatus::Failed,
							latest_progress.as_ref(),
							Some(e.to_string()),
						)
						.await
					{
						error!("Failed to update job failure status in database: {}", e_db);
					}

					Err(e)
				}
			}
		};

		// Clean up checkpoint if job completed
		match &result {
			Ok(_) => {
				let _ = self
					.state
					.checkpoint_handler
					.delete_checkpoint(self.state.job_id)
					.await;
				Ok(ExecStatus::Done(().into()))
			}
			Err(e) => Err(e.clone()),
		}
	}
}

impl<J: JobHandler + std::fmt::Debug> std::fmt::Debug for JobExecutor<J> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("JobExecutor")
			.field("job", &self.job)
			.field("job_id", &self.state.job_id)
			.finish()
	}
}

impl<J: JobHandler + std::fmt::Debug> ErasedJob for JobExecutor<J> {
	fn create_executor(
		self: Box<Self>,
		job_id: JobId,
		library: std::sync::Arc<crate::library::Library>,
		job_db: std::sync::Arc<crate::infra::job::database::JobDb>,
		status_tx: tokio::sync::watch::Sender<JobStatus>,
		progress_tx: tokio::sync::mpsc::UnboundedSender<Progress>,
		broadcast_tx: tokio::sync::broadcast::Sender<Progress>,
		checkpoint_handler: std::sync::Arc<dyn CheckpointHandler>,
		output_handle: Arc<Mutex<Option<JobResult<JobOutput>>>>,
		networking: Option<std::sync::Arc<crate::service::network::NetworkingService>>,
		volume_manager: Option<std::sync::Arc<crate::volume::VolumeManager>>,
		job_logging_config: Option<crate::config::JobLoggingConfig>,
		job_logs_dir: Option<std::path::PathBuf>,
	) -> Box<dyn sd_task_system::Task<JobError>> {
		// Update the executor's state with the new parameters
		let mut executor = *self;
		// Create file logger if job logging is enabled
		let file_logger = if let (Some(config), Some(logs_dir)) = (&job_logging_config, &job_logs_dir) {
			let log_file = logs_dir.join(format!("{}.log", job_id));
			match super::logger::FileJobLogger::new(job_id, log_file, config.clone()) {
				Ok(logger) => {
					let _ = logger.log("INFO", &format!("Job {} starting (via create_executor)", job_id));
					Some(Arc::new(logger))
				}
				Err(e) => {
					error!("Failed to create job logger: {}", e);
					None
				}
			}
		} else {
			None
		};

		executor.state = JobExecutorState {
			job_id,
			library,
			job_db,
			status_tx,
			progress_tx,
			broadcast_tx,
			checkpoint_handler,
			metrics: Default::default(),
			output: output_handle,
			networking,
			volume_manager,
			latest_progress: Arc::new(Mutex::new(None)),
			job_logging_config,
			job_logs_dir,
			file_logger,
		};

		Box::new(executor)
	}

	fn serialize_state(&self) -> Result<Vec<u8>, JobError> {
		rmp_serde::to_vec(&self.job).map_err(|e| JobError::serialization(format!("{}", e)))
	}
}
