//! Job handle for controlling running jobs

use super::{
	error::{JobError, JobResult},
	output::JobOutput,
	progress::Progress,
	types::{JobId, JobStatus},
};
use sd_task_system::TaskHandle;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;
use tokio::sync::{broadcast, watch, Mutex};

/// Handle to a running job
#[derive(Debug)]
pub struct JobHandle {
	pub id: JobId,
	pub job_name: String,
	pub(crate) task_handle: Arc<Mutex<Option<TaskHandle<JobError>>>>,
	pub(crate) status_rx: watch::Receiver<JobStatus>,
	pub(crate) progress_rx: broadcast::Receiver<Progress>,
	pub(crate) output: Arc<Mutex<Option<JobResult<JobOutput>>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct JobReceipt {
	pub id: JobId,
	pub job_name: String,
}

impl From<JobHandle> for JobReceipt {
	fn from(handle: JobHandle) -> Self {
		Self {
			id: handle.id,
			job_name: handle.job_name,
		}
	}
}

impl From<&JobHandle> for JobReceipt {
	fn from(handle: &JobHandle) -> Self {
		Self {
			id: handle.id,
			job_name: handle.job_name.clone(),
		}
	}
}

impl JobHandle {
	pub fn new(
		id: JobId,
		job_name: String,
		task_handle: Arc<Mutex<Option<TaskHandle<JobError>>>>,
		status_rx: watch::Receiver<JobStatus>,
		progress_rx: broadcast::Receiver<Progress>,
		output: Arc<Mutex<Option<JobResult<JobOutput>>>>,
	) -> Self {
		Self {
			id,
			job_name,
			task_handle,
			status_rx,
			progress_rx,
			output,
		}
	}

	/// Convert to serializable JobReceipt for API responses
	pub fn to_receipt(&self) -> JobReceipt {
		JobReceipt {
			id: self.id,
			job_name: self.job_name.clone(),
		}
	}
}

impl Clone for JobHandle {
	fn clone(&self) -> Self {
		Self {
			id: self.id,
			job_name: self.job_name.clone(),
			task_handle: self.task_handle.clone(),
			status_rx: self.status_rx.clone(),
			progress_rx: self.progress_rx.resubscribe(),
			output: self.output.clone(),
		}
	}
}

impl JobHandle {
	/// Get the job ID
	pub fn id(&self) -> JobId {
		self.id
	}

	/// Get the current status
	pub fn status(&self) -> JobStatus {
		*self.status_rx.borrow()
	}

	/// Subscribe to status updates
	pub fn subscribe_status(&self) -> watch::Receiver<JobStatus> {
		self.status_rx.clone()
	}

	/// Subscribe to progress updates
	pub fn subscribe_progress(&self) -> broadcast::Receiver<Progress> {
		self.progress_rx.resubscribe()
	}

	/// Wait for the job to complete
	pub async fn wait(&self) -> JobResult<JobOutput> {
		// Wait for terminal status
		let mut status_rx = self.status_rx.clone();
		while !status_rx.borrow().is_terminal() {
			status_rx
				.changed()
				.await
				.map_err(|_| JobError::Other("Status channel closed".into()))?;
		}

		// Check final status
		let final_status = *status_rx.borrow();
		match final_status {
			JobStatus::Completed => {
				// Get output
				let result = self.output.lock().await.clone();
				match result {
					Some(Ok(output)) => Ok(output),
					Some(Err(e)) => Err(e),
					None => Ok(JobOutput::Success),
				}
			}
			JobStatus::Failed => Err(JobError::ExecutionFailed("Job failed".into())),
			JobStatus::Cancelled => Err(JobError::Interrupted),
			_ => unreachable!("Non-terminal status after wait"),
		}
	}

	/// Pause the job
	pub async fn pause(&self) -> JobResult<()> {
		// For now, these operations need to be implemented through JobManager
		// since the TaskHandle is stored there, not in JobHandle
		todo!("Job control operations will be implemented through JobManager")
	}

	/// Resume the job
	pub async fn resume(&self) -> JobResult<()> {
		todo!("Job control operations will be implemented through JobManager")
	}

	/// Cancel the job
	pub async fn cancel(&self) -> JobResult<()> {
		todo!("Job control operations will be implemented through JobManager")
	}

	/// Force abort the job
	pub async fn force_abort(&self) -> JobResult<()> {
		todo!("Job control operations will be implemented through JobManager")
	}
}

// Serialize JobHandle as its JobId for wire transport
impl serde::Serialize for JobHandle {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer,
	{
		self.id.serialize(serializer)
	}
}

/// Update events from a job
#[derive(Debug)]
pub enum JobUpdate {
	/// Status changed
	StatusChanged(JobStatus),
	/// Progress update
	Progress(Progress),
	/// Job completed
	Completed(JobOutput),
	/// Job failed
	Failed(JobError),
}

impl JobHandle {
	/// Subscribe to all job updates
	pub fn subscribe(&self) -> JobUpdateStream {
		JobUpdateStream {
			handle: self.clone(),
			status_rx: self.status_rx.clone(),
			progress_rx: self.progress_rx.resubscribe(),
		}
	}
}

/// Stream of job updates
pub struct JobUpdateStream {
	handle: JobHandle,
	status_rx: watch::Receiver<JobStatus>,
	progress_rx: broadcast::Receiver<Progress>,
}

impl JobUpdateStream {
	/// Get the next update
	pub async fn next(&mut self) -> Option<JobUpdate> {
		tokio::select! {
			// Status changes
			Ok(_) = self.status_rx.changed() => {
				let status = *self.status_rx.borrow();

				match status {
					JobStatus::Completed => {
						let result = self.handle.output.lock().await.clone();
						match result {
							Some(Ok(output)) => Some(JobUpdate::Completed(output)),
							Some(Err(e)) => Some(JobUpdate::Failed(e)),
							None => Some(JobUpdate::Completed(JobOutput::Success)),
						}
					}
					JobStatus::Failed => {
						Some(JobUpdate::Failed(JobError::ExecutionFailed("Job failed".into())))
					}
					_ => Some(JobUpdate::StatusChanged(status)),
				}
			}

			// Progress updates
			Ok(progress) = self.progress_rx.recv() => {
				Some(JobUpdate::Progress(progress))
			}

			else => None,
		}
	}
}
