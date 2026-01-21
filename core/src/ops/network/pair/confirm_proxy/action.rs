use std::sync::Arc;

use super::{input::PairConfirmProxyInput, output::PairConfirmProxyOutput};
use crate::infra::action::{error::ActionError, CoreAction};

pub struct PairConfirmProxyAction {
	pub session_id: uuid::Uuid,
	pub accepted: bool,
}

impl CoreAction for PairConfirmProxyAction {
	type Output = PairConfirmProxyOutput;
	type Input = PairConfirmProxyInput;

	fn from_input(input: Self::Input) -> std::result::Result<Self, String> {
		Ok(Self {
			session_id: input.session_id,
			accepted: input.accepted,
		})
	}

	async fn execute(
		self,
		context: Arc<crate::context::CoreContext>,
	) -> std::result::Result<Self::Output, ActionError> {
		let net = context
			.get_networking()
			.await
			.ok_or_else(|| ActionError::Internal("Networking not initialized".to_string()))?;

		let registry = net.protocol_registry();
		let guard = registry.read().await;
		if let Some(handler) = guard.get_handler("pairing") {
			if let Some(pairing) = handler
				.as_any()
				.downcast_ref::<crate::service::network::protocol::PairingProtocolHandler>(
			) {
				let result = pairing
					.confirm_proxy_pairing(self.session_id, self.accepted)
					.await;

				match result {
					Ok(_) => {
						return Ok(PairConfirmProxyOutput {
							success: true,
							error: None,
						});
					}
					Err(e) => {
						return Ok(PairConfirmProxyOutput {
							success: false,
							error: Some(e.to_string()),
						});
					}
				}
			}
		}

		Ok(PairConfirmProxyOutput {
			success: false,
			error: Some("Pairing handler not available".to_string()),
		})
	}

	fn action_kind(&self) -> &'static str {
		"network.pair.confirmProxy"
	}
}

crate::register_core_action!(PairConfirmProxyAction, "network.pair.confirmProxy");
