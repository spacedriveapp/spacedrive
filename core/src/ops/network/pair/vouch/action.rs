use std::sync::Arc;

use super::{input::PairVouchInput, output::PairVouchOutput};
use crate::infra::action::{error::ActionError, CoreAction};

pub struct PairVouchAction {
	pub session_id: uuid::Uuid,
	pub target_device_ids: Vec<uuid::Uuid>,
}

impl CoreAction for PairVouchAction {
	type Output = PairVouchOutput;
	type Input = PairVouchInput;

	fn from_input(input: Self::Input) -> std::result::Result<Self, String> {
		Ok(Self {
			session_id: input.session_id,
			target_device_ids: input.target_device_ids,
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
				let session = pairing
					.start_proxy_vouching(self.session_id, self.target_device_ids)
					.await
					.map_err(|e| ActionError::Internal(e.to_string()))?;

				let pending_count = session
					.vouches
					.iter()
					.filter(|v| {
						matches!(
							v.status,
							crate::service::network::protocol::pairing::VouchStatus::Queued
								| crate::service::network::protocol::pairing::VouchStatus::Waiting
								| crate::service::network::protocol::pairing::VouchStatus::Selected
						)
					})
					.count() as u32;

				return Ok(PairVouchOutput {
					success: true,
					pending_count,
				});
			}
		}

		Err(ActionError::Internal(
			"Pairing handler not available".to_string(),
		))
	}

	fn action_kind(&self) -> &'static str {
		"network.pair.vouch"
	}
}

crate::register_core_action!(PairVouchAction, "network.pair.vouch");
