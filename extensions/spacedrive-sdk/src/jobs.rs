//! Job system operations
//!
//! Dispatch and monitor background jobs.

use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::sync::Arc;
use uuid::Uuid;

use crate::ffi::WireClient;
use crate::types::Result;

/// Job client for background task management
pub struct JobClient {
	client: Arc<RefCell<WireClient>>,
}

impl JobClient {
	pub(crate) fn new(client: Arc<RefCell<WireClient>>) -> Self {
		Self { client }
	}

	/// Dispatch a background job
	pub fn dispatch(&self, job_type: &str, params: serde_json::Value) -> Result<Uuid> {
		let result: DispatchOutput = self.client.borrow().call(
			"action:jobs.dispatch.input.v1",
			&DispatchInput {
				job_type: job_type.to_string(),
				params,
			},
		)?;
		Ok(result.job_id)
	}

	/// Get job status
	pub fn get_status(&self, job_id: Uuid) -> Result<JobStatus> {
		self.client
			.borrow()
			.call("query:jobs.get_status.v1", &GetJobStatus { job_id })
	}

	/// Cancel a running job
	pub fn cancel(&self, job_id: Uuid) -> Result<()> {
		self.client
			.borrow()
			.call("action:jobs.cancel.input.v1", &CancelJob { job_id })
	}
}

// === Types ===

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JobStatus {
	Queued,
	Running { progress: f32 },
	Completed,
	Failed { error: String },
	Cancelled,
}

#[derive(Debug, Serialize, Deserialize)]
struct DispatchInput {
	job_type: String,
	params: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct DispatchOutput {
	job_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
struct GetJobStatus {
	job_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
struct CancelJob {
	job_id: Uuid,
}
