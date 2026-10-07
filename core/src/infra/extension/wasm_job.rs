//! WASM Job Executor
//!
//! One core job type runs every extension job. The guest's state is an opaque
//! JSON string the guest serializes itself, so the core persists it without
//! knowing its shape: into the job row at dispatch and pause, and into the
//! checkpoint table every time the guest asks.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::infra::job::{error::JobError, prelude::*};

use super::host_functions::{JobBridge, JobEvent};

/// Generic job for executing WASM extension jobs
#[derive(Debug, Serialize, Deserialize, Job)]
pub struct WasmJob {
	/// Extension ID
	pub extension_id: String,

	/// Name the extension registered the job under (e.g. "counter")
	pub job_name: String,

	/// WASM export function name (e.g., "execute_test_counter")
	pub export_fn: String,

	/// Job state as JSON string, as the guest last checkpointed it
	pub state_json: String,

	/// For resumability - track if this is a resumed job
	#[serde(skip)]
	pub is_resuming: bool,
}

impl Job for WasmJob {
	const NAME: &'static str = "wasm_job";
	const RESUMABLE: bool = true;
	const VERSION: u32 = 1;
	const DESCRIPTION: Option<&'static str> = Some("Execute WASM extension job");
}

impl crate::infra::job::traits::DynJob for WasmJob {
	fn job_name(&self) -> &'static str {
		Self::NAME
	}
}

impl WasmJob {
	async fn handle(&mut self, event: JobEvent, ctx: &JobContext<'_>) {
		match event {
			JobEvent::Progress { fraction, message } => {
				ctx.progress(Progress::percentage(fraction));
			}
			JobEvent::Checkpoint { state, saved } => {
				let result = ctx.save_state(&state).await;
				if let Err(e) = &result {
					ctx.log(format!("Checkpoint not saved: {e}"));
				}
				self.state_json = state;
				let _ = saved.send(result.is_ok());
			}
			JobEvent::Warning(message) => ctx.add_warning(message),
			JobEvent::Items(count) => ctx.increment_items(count).await,
			JobEvent::Bytes(bytes) => ctx.increment_bytes(bytes).await,
		}
	}
}

#[async_trait::async_trait]
impl JobHandler for WasmJob {
	type Output = JobOutput;

	async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
		let plugin_manager = ctx
			.library()
			.core_context()
			.get_plugin_manager()
			.await
			.ok_or_else(|| JobError::ExecutionFailed("PluginManager not initialized".into()))?;
		let runtime = plugin_manager
			.read()
			.await
			.runtime(&self.extension_id)
			.await
			.ok_or_else(|| {
				JobError::ExecutionFailed(format!("Extension '{}' not loaded", self.extension_id))
			})?;

		// A checkpoint outlives a kill; the job row only outlives a pause.
		if let Some(saved) = ctx.load_state::<String>().await? {
			ctx.log("Resuming from checkpoint");
			self.state_json = saved;
		}

		ctx.log(format!(
			"Running {}:{} ({})",
			self.extension_id, self.job_name, self.export_fn
		));

		let ctx_json = serde_json::json!({
			"job_id": ctx.id().to_string(),
			"library_id": ctx.library().id().to_string(),
		})
		.to_string();

		let (events_tx, mut events_rx) = mpsc::unbounded_channel();
		let interrupted = Arc::new(AtomicBool::new(false));
		let bridge = JobBridge {
			events: events_tx,
			interrupted: interrupted.clone(),
		};

		// The guest blocks its thread for the whole job, so it runs off the
		// runtime while this task feeds its host calls back into the context.
		let export_fn = self.export_fn.clone();
		let state_json = self.state_json.clone();
		let mut guest = tokio::task::spawn_blocking(move || {
			let mut runtime = runtime
				.lock()
				.map_err(|_| "plugin runtime lock poisoned".to_string())?;
			runtime
				.run_job(&export_fn, &ctx_json, &state_json, bridge)
				.map_err(|e| e.to_string())
		});

		let mut interrupt_poll = tokio::time::interval(Duration::from_millis(50));
		let exit = loop {
			tokio::select! {
				result = &mut guest => break result,
				event = events_rx.recv() => {
					if let Some(event) = event {
						self.handle(event, &ctx).await;
					}
				}
				_ = interrupt_poll.tick() => {
					if ctx.check_interrupt().await.is_err() {
						interrupted.store(true, Ordering::SeqCst);
					}
				}
			}
		};
		while let Ok(event) = events_rx.try_recv() {
			self.handle(event, &ctx).await;
		}

		match exit {
			Ok(Ok(0)) => Ok(JobOutput::Success),
			Ok(Ok(1)) => Err(JobError::Interrupted),
			Ok(Ok(code)) => Err(JobError::ExecutionFailed(format!(
				"extension job exited with code {code}"
			))),
			Ok(Err(e)) => Err(JobError::ExecutionFailed(e)),
			Err(e) => Err(JobError::ExecutionFailed(format!(
				"extension job thread failed: {e}"
			))),
		}
	}

	async fn on_resume(&mut self, ctx: &JobContext<'_>) -> JobResult<()> {
		self.is_resuming = true;
		ctx.log("Resuming WASM job");
		Ok(())
	}

	fn is_resuming(&self) -> bool {
		self.is_resuming
	}
}
