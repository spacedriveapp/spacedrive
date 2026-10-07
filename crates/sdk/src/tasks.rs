//! Task system for job composition
//!
//! A task is a unit of work a job runs through [`crate::job_context::JobContext::run`].
//! The `#[task]` attribute turns an async function into a unit struct that
//! implements [`Task`], carrying the retry and timeout policy the attribute
//! declares. The policy is enforced by the host: `task.begin` hands the host
//! the deadline, and every host operation the task makes after the deadline
//! fails with [`Error::Timeout`], which is the only way a single-threaded
//! guest can be stopped mid-task. Retries are the guest's loop over attempts.

use serde::{Deserialize, Serialize};

use crate::types::*;
use crate::{ai::AiContext, vdfs::VdfsContext};

/// A unit of work with a retry and timeout policy.
///
/// Implemented by the `#[task]` attribute; extensions do not implement it by
/// hand.
pub trait Task {
	type Args;
	type Output;

	/// The function's name, as the job log reports it.
	const NAME: &'static str;
	/// Attempts after the first one, on a retryable error.
	const RETRIES: u32;
	/// Deadline per attempt; 0 means none.
	const TIMEOUT_MS: u64;

	async fn call(ctx: TaskContext, args: Self::Args) -> TaskResult<Self::Output>;
}

/// What the guest tells the host when a task attempt starts.
#[derive(Serialize)]
pub(crate) struct TaskBegin<'a> {
	pub name: &'a str,
	pub attempt: u32,
	pub retries: u32,
	pub timeout_ms: u64,
}

/// What the guest tells the host when an attempt ends.
#[derive(Serialize)]
pub(crate) struct TaskEnd<'a> {
	pub name: &'a str,
	pub attempt: u32,
	pub error: Option<&'a str>,
}

#[derive(Deserialize)]
pub(crate) struct TaskHandle {
	pub id: u32,
}

/// Run a task to completion, retrying on retryable errors as its policy says.
pub(crate) async fn run<T: Task>(args: T::Args) -> Result<T::Output>
where
	T::Args: Clone,
{
	let mut attempt = 0;
	loop {
		let handle: TaskHandle = crate::ffi::op_json(
			"task.begin",
			&TaskBegin {
				name: T::NAME,
				attempt,
				retries: T::RETRIES,
				timeout_ms: T::TIMEOUT_MS,
			},
		)?;
		let result = T::call(TaskContext { task_id: handle.id }, args.clone()).await;
		let error = result.as_ref().err().map(|e| e.to_string());
		let _: serde_json::Value = crate::ffi::op_json(
			"task.end",
			&TaskEnd {
				name: T::NAME,
				attempt,
				error: error.as_deref(),
			},
		)?;
		match result {
			Ok(output) => return Ok(output),
			Err(error) if attempt < T::RETRIES && error.is_retryable() => attempt += 1,
			Err(error) => return Err(error),
		}
	}
}

/// Task execution context
pub struct TaskContext {
	task_id: u32,
}

impl TaskContext {
	/// The host's handle for this attempt.
	pub fn task_id(&self) -> u32 {
		self.task_id
	}

	/// Access VDFS
	pub fn vdfs(&self) -> VdfsContext {
		VdfsContext
	}

	/// Access AI
	pub fn ai(&self) -> AiContext {
		AiContext
	}

	/// The extension's configuration: `config.json` beside its manifest,
	/// or `{}` when there is none, so a config type with serde defaults
	/// always loads. A config that does not deserialize is a panic, since
	/// the job cannot do anything sensible without it.
	pub fn config<C: serde::de::DeserializeOwned + 'static>(&self) -> &C {
		crate::config::load()
	}

	/// Read sidecar data
	pub async fn read_sidecar<T: serde::de::DeserializeOwned>(
		&self,
		content_uuid: Uuid,
		kind: &str,
	) -> Result<T> {
		VdfsContext.read_sidecar(content_uuid, kind).await
	}
}

/// Task result type
pub type TaskResult<T> = std::result::Result<T, Error>;
