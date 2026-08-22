//! Register a service with the host's supervisor.

use crate::{
	context::CoreContext,
	infra::action::{error::ActionError, CoreAction},
};
use sd_supervisor::{ServiceDefinition, SupervisorError};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ProcessRegisterInput {
	#[serde(flatten)]
	pub definition: ServiceDefinition,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ProcessRegisterOutput {
	pub name: String,
}

pub struct ProcessRegisterAction {
	definition: ServiceDefinition,
}

impl CoreAction for ProcessRegisterAction {
	type Input = ProcessRegisterInput;
	type Output = ProcessRegisterOutput;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		if input.definition.name.trim().is_empty() {
			return Err("service name cannot be empty".to_string());
		}
		Ok(Self {
			definition: input.definition,
		})
	}

	async fn execute(self, context: Arc<CoreContext>) -> Result<Self::Output, ActionError> {
		let manager = context
			.processes()
			.await
			.ok_or_else(|| ActionError::Internal("process manager not initialized".to_string()))?;
		let name = self.definition.name.clone();
		manager
			.register(self.definition)
			.await
			.map_err(|err| match err {
				SupervisorError::DuplicateService(_) => ActionError::InvalidInput(err.to_string()),
				other => ActionError::Internal(other.to_string()),
			})?;
		Ok(ProcessRegisterOutput { name })
	}

	fn action_kind(&self) -> &'static str {
		"processes.register"
	}
}

crate::register_core_action!(ProcessRegisterAction, "processes.register");
