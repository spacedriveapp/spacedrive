//! Start one supervised service.

use super::resource::Process;
use crate::{
	context::CoreContext,
	infra::action::{error::ActionError, CoreAction},
};
use sd_supervisor::SupervisorError;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ProcessStartInput {
	pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ProcessStartOutput {
	pub process: Process,
}

pub struct ProcessStartAction {
	name: String,
}

pub(super) fn action_error(err: SupervisorError) -> ActionError {
	match err {
		SupervisorError::UnknownService(_)
		| SupervisorError::DuplicateService(_)
		| SupervisorError::ObservedService(_)
		| SupervisorError::AdoptedService(_) => ActionError::InvalidInput(err.to_string()),
		SupervisorError::Runtime(detail) => ActionError::Internal(detail),
	}
}

impl CoreAction for ProcessStartAction {
	type Input = ProcessStartInput;
	type Output = ProcessStartOutput;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		Ok(Self { name: input.name })
	}

	async fn execute(self, context: Arc<CoreContext>) -> Result<Self::Output, ActionError> {
		let manager = context.processes().await.ok_or_else(|| {
			ActionError::Internal("process manager not initialized".to_string())
		})?;
		let status = manager
			.supervisor()
			.start(&self.name)
			.await
			.map_err(action_error)?;
		Ok(ProcessStartOutput {
			process: Process::from(status),
		})
	}

	fn action_kind(&self) -> &'static str {
		"processes.start"
	}
}

crate::register_core_action!(ProcessStartAction, "processes.start");
