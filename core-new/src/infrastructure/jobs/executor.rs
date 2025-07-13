//! Job executor that wraps jobs for task system integration

use super::{
	context::{CheckpointHandler, JobContext},
	database::JobDb,
	error::{JobError, JobResult},
	handle::JobHandle,
	output::JobOutput,
	progress::Progress,
	traits::{Job, JobHandler},
	types::{ErasedJob, JobId, JobMetrics, JobStatus},
};
use crate::library::Library;
use async_trait::async_trait;
use sd_task_system::{ExecStatus, Interrupter, Task, TaskId};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, watch, Mutex};
use tracing::{debug, error, info};

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
	pub output: Option<JobOutput>,
	pub networking: Option<Arc<crate::services::networking::NetworkingService>>,
	pub volume_manager: Option<Arc<crate::volume::VolumeManager>>,
	pub latest_progress: Arc<Mutex<Option<Progress>>>,
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
		networking: Option<Arc<crate::services::networking::NetworkingService>>,
		volume_manager: Option<Arc<crate::volume::VolumeManager>>,
	) -> Self {
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
				output: None,
				networking,
				volume_manager,
				latest_progress: Arc::new(Mutex::new(None)),
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
		info!("Starting job {}: {}", self.state.job_id, J::NAME);

		// Update status to running
		let _ = self.state.status_tx.send(super::types::JobStatus::Running);

		// Also persist status to database
		if let Err(e) = self
			.update_job_status_in_db(super::types::JobStatus::Running)
			.await
		{
			error!("Failed to update job status in database: {}", e);
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
		};

		// Progress forwarding is handled by JobManager

		// Check if we're resuming
		// TODO: Implement proper resume detection
		debug!("Starting job {}", self.state.job_id);

		// Store metrics reference for later update
		let metrics_ref = ctx.metrics.clone();

		// Run the job
		let result = match self.job.run(ctx).await {
			Ok(output) => {
				self.state.output = Some(output.into());

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
				let final_progress = if let Some(ref output) = self.state.output {
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
			Err(e) => {
				if e.is_interrupted() {
					debug!("Job {} interrupted", self.state.job_id);

					// Update status with current progress
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
		if matches!(result, Ok(ExecStatus::Done(_))) {
			let _ = self
				.state
				.checkpoint_handler
				.delete_checkpoint(self.state.job_id)
				.await;
		}

		result
	}
}
