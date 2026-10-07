//! Start a job an extension registered

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use crate::{
	context::CoreContext,
	infra::action::{error::ActionResult, LibraryAction},
};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct RunExtensionJobInput {
	/// `<extension id>:<job name>`, as `extensions.list` reports it
	pub job: String,
	/// The job's initial state, in the shape the extension's state type
	/// deserializes. Omit it to start from the state type's default.
	#[serde(default)]
	pub state: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct RunExtensionJobOutput {
	pub job_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunExtensionJobAction {
	input: RunExtensionJobInput,
}

impl LibraryAction for RunExtensionJobAction {
	type Input = RunExtensionJobInput;
	type Output = RunExtensionJobOutput;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		Ok(Self { input })
	}

	fn action_kind(&self) -> &'static str {
		"extensions.run_job"
	}

	async fn execute(
		self,
		library: Arc<crate::library::Library>,
		context: Arc<CoreContext>,
	) -> ActionResult<Self::Output> {
		#[cfg(feature = "wasm")]
		{
			use crate::infra::action::error::ActionError;

			let plugin_manager = context
				.get_plugin_manager()
				.await
				.ok_or_else(|| ActionError::Internal("extensions are not initialized".into()))?;
			let state_json = match self.input.state {
				Some(state) => state.to_string(),
				None => String::new(),
			};
			let job = plugin_manager
				.read()
				.await
				.job_registry()
				.create_wasm_job(&self.input.job, state_json)
				.map_err(ActionError::InvalidInput)?;
			let handle = library
				.jobs()
				.dispatch(job)
				.await
				.map_err(|e| ActionError::Internal(e.to_string()))?;
			Ok(RunExtensionJobOutput {
				job_id: handle.id().0,
			})
		}
		#[cfg(not(feature = "wasm"))]
		{
			let _ = (library, context);
			Err(crate::infra::action::error::ActionError::Internal(
				"this build has no extension runtime (enable the wasm feature)".into(),
			))
		}
	}
}

crate::register_library_action!(RunExtensionJobAction, "extensions.run_job");
