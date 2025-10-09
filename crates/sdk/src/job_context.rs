//! Job execution context for extensions
//!
//! Provides the same capabilities as core jobs: progress, checkpoints, metrics, etc.

use serde::Serialize;

use crate::types::Result;

/// Job-specific imports (will be implemented in core)
#[link(wasm_import_module = "spacedrive")]
extern "C" {
	fn job_report_progress(
		job_id_ptr: *const u8,
		progress: f32,
		message_ptr: *const u8,
		message_len: usize,
	);
	fn job_checkpoint(job_id_ptr: *const u8, state_ptr: *const u8, state_len: usize) -> i32;
	fn job_check_interrupt(job_id_ptr: *const u8) -> i32;
	fn job_add_warning(job_id_ptr: *const u8, message_ptr: *const u8, message_len: usize);
	fn job_increment_bytes(job_id_ptr: *const u8, bytes: u64);
	fn job_increment_items(job_id_ptr: *const u8, count: u64);
}

/// Context for job execution
///
/// Provides access to all job capabilities: progress, checkpoints, metrics, etc.
pub struct JobContext {
	job_id: [u8; 16],     // UUID bytes
	library_id: [u8; 16], // UUID bytes
}

impl JobContext {
	/// Create job context from parameters passed by Core
	pub fn from_params(ctx_json: &str) -> Result<Self> {
		let ctx: JobContextParams = serde_json::from_str(ctx_json)
			.map_err(|e| crate::types::Error::Deserialization(e.to_string()))?;

		// Parse UUIDs from strings
		let job_id = parse_uuid(&ctx.job_id)?;
		let library_id = parse_uuid(&ctx.library_id)?;

		Ok(Self { job_id, library_id })
	}

	/// Get job ID as bytes
	pub fn job_id_bytes(&self) -> &[u8; 16] {
		&self.job_id
	}

	/// Get library ID as bytes
	pub fn library_id_bytes(&self) -> &[u8; 16] {
		&self.library_id
	}

	/// Report progress (0.0 to 1.0)
	pub fn report_progress(&self, progress: f32, message: &str) {
		unsafe {
			job_report_progress(
				self.job_id.as_ptr(),
				progress,
				message.as_ptr(),
				message.len(),
			);
		}
	}

	/// Save checkpoint with current state
	pub fn checkpoint<S: Serialize>(&self, state: &S) -> Result<()> {
		let state_bytes = serde_json::to_vec(state)
			.map_err(|e| crate::types::Error::Serialization(e.to_string()))?;

		let result = unsafe {
			job_checkpoint(
				self.job_id.as_ptr(),
				state_bytes.as_ptr(),
				state_bytes.len(),
			)
		};

		if result == 0 {
			Ok(())
		} else {
			Err(crate::types::Error::OperationFailed(
				"Checkpoint failed".into(),
			))
		}
	}

	/// Check if job should pause or cancel
	/// Returns true if interrupted
	pub fn check_interrupt(&self) -> bool {
		let result = unsafe { job_check_interrupt(self.job_id.as_ptr()) };
		result != 0
	}

	/// Add a warning (non-fatal issue)
	pub fn add_warning(&self, message: &str) {
		unsafe {
			job_add_warning(self.job_id.as_ptr(), message.as_ptr(), message.len());
		}
	}

	/// Track bytes processed (for metrics)
	pub fn increment_bytes(&self, bytes: u64) {
		unsafe {
			job_increment_bytes(self.job_id.as_ptr(), bytes);
		}
	}

	/// Track items processed (for metrics)
	pub fn increment_items(&self, count: u64) {
		unsafe {
			job_increment_items(self.job_id.as_ptr(), count);
		}
	}

	// VDFS, AI, Credentials removed - operations don't exist yet
	// Will be added back once core operations are implemented

	/// Log a message
	pub fn log(&self, message: &str) {
		crate::ffi::log_info(message);
	}

	/// Log an error
	pub fn log_error(&self, message: &str) {
		crate::ffi::log_error(message);
	}
}

/// Parameters passed from Core to WASM job
#[derive(serde::Deserialize)]
struct JobContextParams {
	job_id: String,
	library_id: String,
}

/// Job execution result
pub enum JobResult {
	Completed,
	Interrupted,
	Failed(String),
}

impl JobResult {
	/// Return code for completed job
	pub fn to_exit_code(&self) -> i32 {
		match self {
			JobResult::Completed => 0,
			JobResult::Interrupted => 1,
			JobResult::Failed(_) => 2,
		}
	}
}

fn parse_uuid(s: &str) -> Result<[u8; 16]> {
	// Simple UUID parsing (hyphen-separated hex)
	let s = s.replace("-", "");
	if s.len() != 32 {
		return Err(crate::types::Error::InvalidInput("Invalid UUID".into()));
	}

	let mut bytes = [0u8; 16];
	for i in 0..16 {
		let byte_str = &s[i * 2..i * 2 + 2];
		bytes[i] = u8::from_str_radix(byte_str, 16)
			.map_err(|_| crate::types::Error::InvalidInput("Invalid UUID hex".into()))?;
	}

	Ok(bytes)
}
