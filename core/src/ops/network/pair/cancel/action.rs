use super::{input::PairCancelInput, output::PairCancelOutput};
use crate::infra::action::{error::ActionError, CoreAction};
use std::sync::Arc;

pub struct PairCancelAction { pub session_id: uuid::Uuid }

impl CoreAction for PairCancelAction {
	type Output = PairCancelOutput;
	type Input = PairCancelInput;

	fn from_input(input: Self::Input) -> std::result::Result<Self, String> { Ok(Self { session_id: input.session_id }) }

	async fn execute(self, context: Arc<crate::context::CoreContext>) -> std::result::Result<Self::Output, ActionError> {
		let net = context.get_networking().await.ok_or_else(|| ActionError::Internal("Networking not initialized".to_string()))?;
		// Cancel via pairing protocol handler if available
		let reg = net.protocol_registry();
		let guard = reg.read().await;
		if let Some(handler) = guard.get_handler("pairing") {
			if let Some(pairing) = handler
				.as_any()
				.downcast_ref::<crate::service::network::protocol::PairingProtocolHandler>()
			{
				pairing.cancel_session(self.session_id).await.map_err(|e| ActionError::Internal(e.to_string()))?;
				return Ok(PairCancelOutput { cancelled: true });
			}
		}
		Ok(PairCancelOutput { cancelled: false })
	}

	fn action_kind(&self) -> &'static str { "network.pair.cancel" }
}

crate::register_core_action!(PairCancelAction, "network.pair.cancel");

