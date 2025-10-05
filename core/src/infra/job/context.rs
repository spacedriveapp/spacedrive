//! Job execution context

use super::{
	error::{JobError, JobResult},
	handle::JobHandle,
	progress::Progress,
	types::{JobId, JobMetrics},
};
use crate::{library::Library, service::network::NetworkingService};
use sd_task_system::Interrupter;
use sea_orm::DatabaseConnection;
use serde::{de::DeserializeOwned, Serialize};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};
use tracing::{debug, warn};

/// Context provided to jobs during execution
pub struct JobContext<'a> {
	pub(crate) id: JobId,
	pub(crate) library: Arc<Library>,
	pub(crate) interrupter: &'a Interrupter,
	pub(crate) progress_tx: mpsc::UnboundedSender<Progress>,
	pub(crate) metrics: Arc<Mutex<JobMetrics>>,
	pub(crate) checkpoint_handler: Arc<dyn CheckpointHandler>,
	pub(crate) child_handles: Arc<Mutex<Vec<JobHandle>>>,
	pub(crate) networking: Option<Arc<NetworkingService>>,
	pub(crate) volume_manager: Option<Arc<crate::volume::VolumeManager>>,
	pub(crate) file_logger: Option<Arc<super::logger::FileJobLogger>>,
}

impl<'a> JobContext<'a> {
	/// Get the job ID
	pub fn id(&self) -> JobId {
		self.id
	}

	/// Get the library this job is running in
	pub fn library(&self) -> &Library {
		&self.library
	}

	/// Get the library database connection
	pub fn library_db(&self) -> &DatabaseConnection {
		self.library.db().conn()
	}

	/// Get networking service if available
	pub fn networking_service(&self) -> Option<Arc<NetworkingService>> {
		self.networking.clone()
	}

	/// Get volume manager if available
	pub fn volume_manager(&self) -> Option<Arc<crate::volume::VolumeManager>> {
		self.volume_manager.clone()
	}

	/// Report progress
	pub fn progress(&self, progress: Progress) {
		// Log progress messages to file if enabled
		if let Some(logger) = &self.file_logger {
			let _ = logger.log("PROGRESS", &progress.to_string());
		}

		if let Err(e) = self.progress_tx.send(progress) {
			warn!("Failed to send progress update: {}", e);
		}
	}

	/// Add a warning message
	pub fn add_warning(&self, warning: impl Into<String>) {
		let msg = warning.into();

		// Log to file if enabled
		if let Some(logger) = &self.file_logger {
			let _ = logger.log("WARN", &msg);
		}

		self.progress(Progress::indeterminate(format!("{}", msg)));
	}

	/// Add a non-critical error
	pub fn add_non_critical_error(&self, error: impl Into<JobError>) {
		let error_msg = error.into().to_string();

		// Log to file if enabled
		if let Some(logger) = &self.file_logger {
			let _ = logger.log("ERROR", &error_msg);
		}

		self.progress(Progress::indeterminate(format!("{}", error_msg)));

		// Increment error count
		if let Ok(mut metrics) = self.metrics.try_lock() {
			metrics.non_critical_errors_count += 1;
		}
	}

	/// Get current metrics
	pub async fn metrics(&self) -> JobMetrics {
		self.metrics.lock().await.clone()
	}

	/// Increment bytes processed
	pub async fn increment_bytes(&self, bytes: u64) {
		self.metrics.lock().await.bytes_processed += bytes;
	}

	/// Increment items processed
	pub async fn increment_items(&self, count: u64) {
		self.metrics.lock().await.items_processed += count;
	}

	/// Check if the job should be interrupted
	pub async fn check_interrupt(&self) -> JobResult<()> {
		if let Some(kind) = self.interrupter.try_check_interrupt() {
			debug!("Job {} received interrupt signal: {:?}", self.id, kind);
			return Err(JobError::Interrupted);
		}
		Ok(())
	}

	/// Create a checkpoint (job can be resumed from here)
	pub async fn checkpoint(&self) -> JobResult<()> {
		self.check_interrupt().await?;
		self.checkpoint_handler.save_checkpoint(self.id, None).await
	}

	/// Create a checkpoint with custom state
	pub async fn checkpoint_with_state<S: Serialize>(&self, state: &S) -> JobResult<()> {
		self.check_interrupt().await?;
		let data = rmp_serde::to_vec(state).map_err(|e| JobError::serialization(e))?;
		self.checkpoint_handler
			.save_checkpoint(self.id, Some(data))
			.await
	}

	/// Load saved state
	pub async fn load_state<S: DeserializeOwned>(&self) -> JobResult<Option<S>> {
		match self.checkpoint_handler.load_checkpoint(self.id).await? {
			Some(data) => {
				let state = rmp_serde::from_slice(&data).map_err(|e| JobError::serialization(e))?;
				Ok(Some(state))
			}
			None => Ok(None),
		}
	}

	/// Save state (without creating a checkpoint)
	pub async fn save_state<S: Serialize>(&self, state: &S) -> JobResult<()> {
		let data = rmp_serde::to_vec(state).map_err(|e| JobError::serialization(e))?;
		self.checkpoint_handler
			.save_checkpoint(self.id, Some(data))
			.await
	}

	/// Spawn a child job
	pub async fn spawn_child<J>(&self, job: J) -> JobResult<JobHandle>
	where
		J: super::traits::Job + super::traits::JobHandler,
	{
		// This will be implemented by JobManager
		// For now, return a placeholder
		todo!("Child job spawning will be implemented with JobManager")
	}

	/// Wait for all child jobs to complete
	pub async fn wait_for_children(&self) -> JobResult<()> {
		let handles = self.child_handles.lock().await.clone();

		for handle in handles {
			handle.wait().await?;
		}

		Ok(())
	}

	/// Log a message
	pub fn log(&self, message: impl Into<String>) {
		let msg = message.into();
		debug!(job_id = %self.id, "{}", msg);

		// Also log to file if enabled
		if let Some(logger) = &self.file_logger {
			let _ = logger.log("INFO", &msg);
		}
	}

	/// Log a debug message
	pub fn log_debug(&self, message: impl Into<String>) {
		let msg = message.into();
		debug!(job_id = %self.id, "{}", msg);

		if let Some(logger) = &self.file_logger {
			let _ = logger.log("DEBUG", &msg);
		}
	}
}

/// Handler for checkpoint operations
#[async_trait::async_trait]
pub trait CheckpointHandler: Send + Sync {
	/// Save a checkpoint
	async fn save_checkpoint(&self, job_id: JobId, data: Option<Vec<u8>>) -> JobResult<()>;

	/// Load a checkpoint
	async fn load_checkpoint(&self, job_id: JobId) -> JobResult<Option<Vec<u8>>>;

	/// Delete a checkpoint
	async fn delete_checkpoint(&self, job_id: JobId) -> JobResult<()>;
}
