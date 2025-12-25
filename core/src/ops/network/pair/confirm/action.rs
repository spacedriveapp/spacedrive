use super::{input::PairConfirmInput, output::PairConfirmOutput};
use crate::infra::action::{error::ActionError, CoreAction};
use std::sync::Arc;

/// Action to confirm or reject a pairing request
///
/// This action is called when the user responds to a pairing confirmation dialog.
/// It either accepts (sending the Challenge to proceed with pairing) or rejects
/// (sending a Reject message to the joiner).
pub struct PairConfirmAction {
	pub session_id: uuid::Uuid,
	pub accepted: bool,
}

impl CoreAction for PairConfirmAction {
	type Output = PairConfirmOutput;
	type Input = PairConfirmInput;

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

		// Get pairing protocol handler
		let reg = net.protocol_registry();
		let guard = reg.read().await;
		let handler = guard
			.get_handler("pairing")
			.ok_or_else(|| ActionError::Internal("Pairing handler not found".to_string()))?;

		let pairing = handler
			.as_any()
			.downcast_ref::<crate::service::network::protocol::PairingProtocolHandler>()
			.ok_or_else(|| ActionError::Internal("Invalid pairing handler type".to_string()))?;

		// Call the user confirmation handler
		match pairing
			.handle_user_confirmation(self.session_id, self.accepted)
			.await
		{
			Ok(response_data) => {
				// If we got response data (Challenge or Reject message), send it to the joiner
				if let Some(data) = response_data {
					// Get the remote node ID from the session to send the response
					let sessions = pairing.get_active_sessions().await;
					let session = sessions
						.iter()
						.find(|s| s.id == self.session_id)
						.ok_or_else(|| {
							ActionError::Internal(format!(
								"Session {} not found after confirmation",
								self.session_id
							))
						})?;

					let info = session.remote_device_info.as_ref().ok_or_else(|| {
						ActionError::Internal("Remote device info not found in session".to_string())
					})?;

					let node_id = info
						.network_fingerprint
						.node_id
						.parse::<iroh::NodeId>()
						.map_err(|e| {
							ActionError::Internal(format!("Failed to parse remote node ID: {}", e))
						})?;

					let endpoint = net.endpoint().ok_or_else(|| {
						ActionError::Internal("Network endpoint not available".to_string())
					})?;

					let msg: crate::service::network::protocol::pairing::PairingMessage =
						serde_json::from_slice(&data).map_err(|e| {
							ActionError::Internal(format!("Failed to parse response: {}", e))
						})?;

					if let Err(e) = pairing
						.send_pairing_message_to_node(endpoint, node_id, &msg)
						.await
					{
						return Ok(PairConfirmOutput {
							success: false,
							error: Some(format!("Failed to send response: {}", e)),
						});
					}
				}

				Ok(PairConfirmOutput {
					success: true,
					error: None,
				})
			}
			Err(e) => Ok(PairConfirmOutput {
				success: false,
				error: Some(e.to_string()),
			}),
		}
	}

	fn action_kind(&self) -> &'static str {
		"network.pair.confirm"
	}
}

crate::register_core_action!(PairConfirmAction, "network.pair.confirm");
