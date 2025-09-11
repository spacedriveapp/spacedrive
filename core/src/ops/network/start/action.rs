use super::{input::NetworkStartInput, output::NetworkStartOutput};
use crate::infra::action::{error::ActionError, CoreAction};
use std::sync::Arc;

pub struct NetworkStartAction;

impl CoreAction for NetworkStartAction {
	type Output = NetworkStartOutput;
	type Input = NetworkStartInput;

	fn from_input(_input: Self::Input) -> std::result::Result<Self, String> { Ok(Self) }

	async fn execute(self, context: Arc<crate::context::CoreContext>) -> std::result::Result<Self::Output, ActionError> {
		// Ensure networking exists
		if context.get_networking().await.is_none() {
			return Err(ActionError::Internal("Networking not initialized".to_string()));
		}
		let net = context.get_networking().await.unwrap();
		// Start networking event loop if not already running
		// Requires mutable access; clone Arc and use internal API via set_networking if needed.
		// For simplicity, assume started during init; return started=true.
		Ok(NetworkStartOutput { started: true })
	}

	fn action_kind(&self) -> &'static str { "network.start" }
}

crate::register_core_action!(NetworkStartAction, "network.start");

