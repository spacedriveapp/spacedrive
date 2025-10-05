use super::{input::NetworkStopInput, output::NetworkStopOutput};
use crate::infra::action::{error::ActionError, CoreAction};
use std::sync::Arc;

pub struct NetworkStopAction;

impl CoreAction for NetworkStopAction {
	type Output = NetworkStopOutput;
	type Input = NetworkStopInput;

	fn from_input(_input: Self::Input) -> std::result::Result<Self, String> {
		Ok(Self)
	}

	async fn execute(
		self,
		context: Arc<crate::context::CoreContext>,
	) -> std::result::Result<Self::Output, ActionError> {
		if let Some(net) = context.get_networking().await {
			net.shutdown()
				.await
				.map_err(|e| ActionError::Internal(e.to_string()))?;
			return Ok(NetworkStopOutput { stopped: true });
		}
		Ok(NetworkStopOutput { stopped: false })
	}

	fn action_kind(&self) -> &'static str {
		"network.stop"
	}
}

crate::register_core_action!(NetworkStopAction, "network.stop");
