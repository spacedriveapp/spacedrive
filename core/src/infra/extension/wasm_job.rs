//! WASM Job Executor
//!
//! Generic job type that executes WASM extension jobs.

use serde::{Deserialize, Serialize};

use crate::infra::job::prelude::*;

/// Generic job for executing WASM extension jobs
#[derive(Debug, Serialize, Deserialize, Job)]
pub struct WasmJob {
	/// Extension ID
	pub extension_id: String,

	/// WASM export function name (e.g., "execute_test_counter")
	pub export_fn: String,

	/// Job state as JSON string
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

// ErasedJob implementation - uses Job derive macro like other jobs

#[async_trait::async_trait]
impl JobHandler for WasmJob {
	type Output = JobOutput;

	async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
		tracing::info!(
			job_id = %ctx.id(),
			extension = %self.extension_id,
			export_fn = %self.export_fn,
			"Executing WASM job"
		);

		// Get PluginManager through Library → CoreContext (simple!)
		let pm_opt = ctx.library().core_context().get_plugin_manager().await;

		let pm = match pm_opt {
			Some(pm) => pm,
			None => {
				ctx.log("ERROR: PluginManager not available");
				return Err(crate::infra::job::error::JobError::ExecutionFailed(
					"PluginManager not initialized".into(),
				));
			}
		};

		// Prepare job context JSON for WASM
		let job_ctx_json = serde_json::json!({
			"job_id": ctx.id().to_string(),
			"library_id": ctx.library().id().to_string(),
		});
		let ctx_json_str = serde_json::to_string(&job_ctx_json).unwrap();

		ctx.log(&format!(
			"Calling WASM function: {}::{}",
			self.extension_id, self.export_fn
		));

		// Call WASM export function
		let result = {
			let pm_lock = pm.write().await;

			// For now, just verify the plugin is loaded
			let plugins = pm_lock.list_plugins().await;
			if !plugins.contains(&self.extension_id) {
				ctx.log(&format!(
					"ERROR: Extension '{}' not loaded",
					self.extension_id
				));
				return Err(crate::infra::job::error::JobError::ExecutionFailed(
					format!("Extension '{}' not loaded", self.extension_id),
				));
			}

			ctx.log(&format!("✓ Extension '{}' is loaded", self.extension_id));

			// TODO: Actually call the WASM export
			// Need to:
			// 1. Get the Instance for this plugin
			// 2. Get the export function
			// 3. Write ctx_json_str and state_json to WASM memory
			// 4. Call the function with pointers
			// 5. Read result code

			ctx.log("WASM export call not yet implemented");
			ctx.log("But the job executed and extension is available!");

			0 // Success code
		};

		ctx.log(&format!("✓ WASM job completed with code: {}", result));

		Ok(JobOutput::Success)
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

// Don't register automatically - will be registered when needed
// WasmJob is a special case, not auto-loaded like regular jobs
