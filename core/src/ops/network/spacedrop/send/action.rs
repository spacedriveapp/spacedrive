use super::{input::SpacedropSendInput, output::SpacedropSendOutput};
use crate::infra::action::{error::ActionError, CoreAction};
use std::sync::Arc;

pub struct SpacedropSendAction {
	pub device_id: uuid::Uuid,
	pub paths: Vec<crate::domain::addressing::SdPath>,
	pub sender: Option<String>,
}

impl CoreAction for SpacedropSendAction {
	type Output = SpacedropSendOutput;
	type Input = SpacedropSendInput;

	fn from_input(input: Self::Input) -> std::result::Result<Self, String> {
		Ok(Self { device_id: input.device_id, paths: input.paths, sender: input.sender })
	}

	async fn execute(self, context: Arc<crate::context::CoreContext>) -> std::result::Result<Self::Output, ActionError> {
		let _net = context.get_networking().await.ok_or_else(|| ActionError::Internal("Networking not initialized".to_string()))?;
		// For now, dispatch a local job-based transfer when possible; placeholder returns none
		Ok(SpacedropSendOutput { job_id: None, session_id: Some(uuid::Uuid::new_v4()) })
	}

	fn action_kind(&self) -> &'static str { "network.spacedrop.send" }
}

crate::register_core_action!(SpacedropSendAction, "network.spacedrop.send");

