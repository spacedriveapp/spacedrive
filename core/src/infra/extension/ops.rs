//! The operations a guest asks the host for through `spacedrive_op`.
//!
//! Every operation that returns data to a guest lands here, named by a
//! string and carried as bytes. Requests are answered on the job's async
//! side, one at a time, because the guest blocks on each; so the state an
//! operation needs lives on [`JobOps`] for the job's duration and no
//! operation ever contends with another from the same job.
//!
//! Permission is checked here against the manifest, never in the guest.

use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::infra::job::prelude::JobContext;
use crate::library::Library;

use super::types::ExtensionManifest;

/// An error a guest can act on. `code` is the stable part: the SDK maps it
/// onto its own error type, and `message` is for the log.
#[derive(Debug, Clone, Serialize)]
pub struct OpError {
	pub code: &'static str,
	pub message: String,
}

impl OpError {
	pub fn not_found() -> Self {
		Self::new("not_found", "not found")
	}

	pub fn permission_denied(message: impl Into<String>) -> Self {
		Self::new("permission_denied", message)
	}

	pub fn invalid_input(message: impl Into<String>) -> Self {
		Self::new("invalid_input", message)
	}

	pub fn not_available(message: impl Into<String>) -> Self {
		Self::new("not_available", message)
	}

	pub fn failed(message: impl Into<String>) -> Self {
		Self::new("failed", message)
	}

	fn new(code: &'static str, message: impl Into<String>) -> Self {
		Self {
			code,
			message: message.into(),
		}
	}

	pub fn to_json(&self) -> String {
		serde_json::to_string(self)
			.unwrap_or_else(|_| format!(r#"{{"code":"{}","message":"unserializable"}}"#, self.code))
	}
}

pub type OpResult = Result<Vec<u8>, OpError>;

/// A task attempt the guest is inside, as `task.begin` declared it.
struct ActiveTask {
	name: String,
	attempt: u32,
	started: Instant,
	deadline: Option<Instant>,
}

/// The host side of one running extension job.
pub struct JobOps {
	pub extension_id: String,
	pub manifest: Arc<ExtensionManifest>,
	pub library: Arc<Library>,
	task: Option<ActiveTask>,
}

#[derive(Deserialize)]
struct TaskBegin {
	name: String,
	attempt: u32,
	retries: u32,
	timeout_ms: u64,
}

#[derive(Deserialize)]
struct TaskEnd {
	name: String,
	attempt: u32,
	error: Option<String>,
}

impl JobOps {
	pub fn new(
		extension_id: String,
		manifest: Arc<ExtensionManifest>,
		library: Arc<Library>,
	) -> Self {
		Self {
			extension_id,
			manifest,
			library,
			task: None,
		}
	}

	/// Answer one request.
	///
	/// Once the active task's deadline has passed, every operation but the
	/// one that ends the task fails with `timeout`, which is how a
	/// single-threaded guest gets stopped mid-task: it cannot be preempted,
	/// but it cannot make progress without the host either.
	pub async fn handle(&mut self, op: &str, payload: &[u8], ctx: &JobContext<'_>) -> OpResult {
		if op != "task.end" {
			if let Some(task) = &self.task {
				if task
					.deadline
					.is_some_and(|deadline| Instant::now() >= deadline)
				{
					return Err(OpError::new(
						"timeout",
						format!("task {} ran past its deadline", task.name),
					));
				}
			}
		}

		match op {
			"task.begin" => self.task_begin(parse(payload)?, ctx),
			"task.end" => self.task_end(parse(payload)?, ctx),
			_ => Err(OpError::new(
				"unknown_op",
				format!("unknown operation {op}"),
			)),
		}
	}

	fn task_begin(&mut self, begin: TaskBegin, ctx: &JobContext<'_>) -> OpResult {
		if let Some(active) = &self.task {
			return Err(OpError::invalid_input(format!(
				"task {} started while task {} is running",
				begin.name, active.name
			)));
		}
		ctx.log(format!(
			"task {} attempt {}/{} started",
			begin.name,
			begin.attempt + 1,
			begin.retries + 1
		));
		let started = Instant::now();
		self.task = Some(ActiveTask {
			name: begin.name,
			attempt: begin.attempt,
			started,
			deadline: (begin.timeout_ms > 0)
				.then(|| started + Duration::from_millis(begin.timeout_ms)),
		});
		json(&serde_json::json!({ "id": begin.attempt }))
	}

	fn task_end(&mut self, end: TaskEnd, ctx: &JobContext<'_>) -> OpResult {
		let Some(task) = self.task.take() else {
			return Err(OpError::invalid_input(format!(
				"task {} ended without starting",
				end.name
			)));
		};
		let elapsed = task.started.elapsed().as_millis();
		match end.error {
			None => ctx.log(format!(
				"task {} attempt {} finished in {elapsed} ms",
				task.name,
				task.attempt + 1
			)),
			Some(error) => ctx.log(format!(
				"task {} attempt {} failed after {elapsed} ms: {error}",
				task.name,
				task.attempt + 1
			)),
		}
		json(&serde_json::Value::Null)
	}
}

fn parse<T: for<'de> Deserialize<'de>>(payload: &[u8]) -> Result<T, OpError> {
	serde_json::from_slice(payload).map_err(|e| OpError::invalid_input(e.to_string()))
}

fn json<T: Serialize>(value: &T) -> OpResult {
	serde_json::to_vec(value).map_err(|e| OpError::failed(e.to_string()))
}
