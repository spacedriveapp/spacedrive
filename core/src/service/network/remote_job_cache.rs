//! Cache for remote device job states

use crate::infra::job::{generic_progress::GenericProgress, output::JobOutput, types::JobStatus};
use crate::service::network::protocol::job_activity::RemoteJobEvent;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Cache for remote device job states
pub struct RemoteJobCache {
	/// Map of device_id → job_id → job state
	jobs: Arc<RwLock<HashMap<Uuid, HashMap<String, RemoteJobState>>>>,

	/// Last update timestamp per device
	last_update: Arc<RwLock<HashMap<Uuid, DateTime<Utc>>>>,
}

/// State of a job running on a remote device
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct RemoteJobState {
	pub job_id: String,
	pub job_type: String,
	pub library_id: Uuid,
	pub device_id: Uuid,
	pub device_name: String,
	pub status: JobStatus,
	pub progress: Option<f64>,
	pub message: Option<String>,
	pub generic_progress: Option<GenericProgress>,
	pub started_at: Option<DateTime<Utc>>,
	pub completed_at: Option<DateTime<Utc>>,
	pub error: Option<String>,
}

impl RemoteJobCache {
	pub fn new() -> Self {
		Self {
			jobs: Arc::new(RwLock::new(HashMap::new())),
			last_update: Arc::new(RwLock::new(HashMap::new())),
		}
	}

	/// Update cache with a remote job event
	pub async fn handle_event(
		&self,
		device_id: Uuid,
		device_name: String,
		library_id: Uuid,
		event: RemoteJobEvent,
	) {
		let mut jobs = self.jobs.write().await;
		let device_jobs = jobs.entry(device_id).or_insert_with(HashMap::new);

		match event {
			RemoteJobEvent::JobQueued {
				job_id,
				job_type,
				timestamp,
			} => {
				device_jobs.insert(
					job_id.clone(),
					RemoteJobState {
						job_id,
						job_type,
						library_id,
						device_id,
						device_name,
						status: JobStatus::Queued,
						progress: None,
						message: None,
						generic_progress: None,
						started_at: Some(timestamp),
						completed_at: None,
						error: None,
					},
				);
			}

			RemoteJobEvent::JobStarted {
				job_id, timestamp, ..
			} => {
				if let Some(job) = device_jobs.get_mut(&job_id) {
					job.status = JobStatus::Running;
					job.started_at = Some(timestamp);
				}
			}

			RemoteJobEvent::JobProgress {
				job_id,
				progress,
				message,
				generic_progress,
				..
			} => {
				if let Some(job) = device_jobs.get_mut(&job_id) {
					job.progress = Some(progress);
					job.message = message;
					job.generic_progress = generic_progress;
				}
			}

			RemoteJobEvent::JobCompleted {
				job_id, timestamp, ..
			} => {
				if let Some(job) = device_jobs.get_mut(&job_id) {
					job.status = JobStatus::Completed;
					job.completed_at = Some(timestamp);
					job.progress = Some(100.0);
				}
			}

			RemoteJobEvent::JobFailed {
				job_id,
				error,
				timestamp,
				..
			} => {
				if let Some(job) = device_jobs.get_mut(&job_id) {
					job.status = JobStatus::Failed;
					job.error = Some(error);
					job.completed_at = Some(timestamp);
				}
			}

			RemoteJobEvent::JobCancelled {
				job_id, timestamp, ..
			} => {
				if let Some(job) = device_jobs.get_mut(&job_id) {
					job.status = JobStatus::Cancelled;
					job.completed_at = Some(timestamp);
				}
			}

			RemoteJobEvent::JobPaused { job_id, .. } => {
				if let Some(job) = device_jobs.get_mut(&job_id) {
					job.status = JobStatus::Paused;
				}
			}

			RemoteJobEvent::JobResumed { job_id, .. } => {
				if let Some(job) = device_jobs.get_mut(&job_id) {
					job.status = JobStatus::Running;
				}
			}
		}

		self.last_update.write().await.insert(device_id, Utc::now());
	}

	/// Get all active jobs for a device
	pub async fn get_device_jobs(&self, device_id: Uuid) -> Vec<RemoteJobState> {
		let jobs = self.jobs.read().await;
		jobs.get(&device_id)
			.map(|device_jobs| {
				device_jobs
					.values()
					.filter(|job| job.status.is_active())
					.cloned()
					.collect()
			})
			.unwrap_or_default()
	}

	/// Get all active jobs across all devices
	pub async fn get_all_active_jobs(&self) -> HashMap<Uuid, Vec<RemoteJobState>> {
		let jobs = self.jobs.read().await;
		jobs.iter()
			.map(|(device_id, device_jobs)| {
				let active: Vec<RemoteJobState> = device_jobs
					.values()
					.filter(|job| job.status.is_active())
					.cloned()
					.collect();
				(*device_id, active)
			})
			.filter(|(_, jobs)| !jobs.is_empty())
			.collect()
	}

	/// Clean up completed jobs older than threshold
	pub async fn cleanup_old_jobs(&self, max_age: Duration) {
		let now = Utc::now();
		let mut jobs = self.jobs.write().await;

		for device_jobs in jobs.values_mut() {
			device_jobs.retain(|_, job| {
				if job.status.is_terminal() {
					if let Some(completed_at) = job.completed_at {
						let age = now.signed_duration_since(completed_at);
						return age.num_seconds() < max_age.num_seconds();
					}
				}
				true
			});
		}
	}

	/// Remove all jobs for a specific device (on disconnect)
	pub async fn remove_device_jobs(&self, device_id: Uuid) {
		self.jobs.write().await.remove(&device_id);
		self.last_update.write().await.remove(&device_id);
	}
}

impl Default for RemoteJobCache {
	fn default() -> Self {
		Self::new()
	}
}
