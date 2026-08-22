//! Stop one supervised service.

use super::{resource::Process, start::action_error};
use crate::{
	context::CoreContext,
	infra::action::{error::ActionError, CoreAction},
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ProcessStopInput {
	pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ProcessStopOutput {
	pub process: Process,
}

pub struct ProcessStopAction {
	name: String,
}

impl CoreAction for ProcessStopAction {
	type Input = ProcessStopInput;
	type Output = ProcessStopOutput;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		Ok(Self { name: input.name })
	}

	async fn execute(self, context: Arc<CoreContext>) -> Result<Self::Output, ActionError> {
		let manager = context
			.processes()
			.await
			.ok_or_else(|| ActionError::Internal("process manager not initialized".to_string()))?;
		let status = manager
			.supervisor()
			.stop(&self.name)
			.await
			.map_err(action_error)?;
		Ok(ProcessStopOutput {
			process: Process::from(status),
		})
	}

	fn action_kind(&self) -> &'static str {
		"processes.stop"
	}
}

crate::register_core_action!(ProcessStopAction, "processes.stop");
