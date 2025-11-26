use super::{input::PairJoinInput, output::PairJoinOutput};
use crate::infra::action::{error::ActionError, CoreAction};
use std::sync::Arc;

pub struct PairJoinAction {
	pub code: String,
	pub node_id: Option<String>,
}

impl CoreAction for PairJoinAction {
	type Output = PairJoinOutput;
	type Input = PairJoinInput;

	fn from_input(input: Self::Input) -> std::result::Result<Self, String> {
		Ok(Self {
			code: input.code,
			node_id: input.node_id,
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

		// Try to parse as QR code JSON first, fallback to manual word entry
		let mut pairing_code = if self.code.trim().starts_with('{') {
			// Looks like JSON (QR code)
			crate::service::network::protocol::pairing::PairingCode::from_qr_json(&self.code)
				.map_err(|e| ActionError::Internal(format!("Invalid QR code: {}", e)))?
		} else {
			// Looks like manual word entry
			crate::service::network::protocol::pairing::PairingCode::from_string(&self.code)
				.map_err(|e| ActionError::Internal(format!("Invalid pairing code: {}", e)))?
		};

		// If node_id provided separately, add it to enable relay fallback
		if let Some(node_id_str) = &self.node_id {
			let node_id: iroh::NodeId = node_id_str
				.parse()
				.map_err(|e| ActionError::Internal(format!("Invalid node ID: {}", e)))?;
			pairing_code = pairing_code.with_node_id(node_id);
		}

		net.start_pairing_as_joiner_with_code(pairing_code, false)
			.await
			.map_err(|e| ActionError::Internal(e.to_string()))?;
		// Best-effort: fetch pairing sessions and find completed one
		let sessions = net
			.get_pairing_status()
			.await
			.map_err(|e| ActionError::Internal(e.to_string()))?;
		if let Some(s) = sessions
			.into_iter()
			.find(|s| matches!(s.state, crate::service::network::PairingState::Completed))
		{
			let dev = s.remote_device_id.unwrap_or_default();
			let name = s
				.remote_device_info
				.map(|i| i.device_name)
				.unwrap_or_else(|| "Remote Device".to_string());
			Ok(PairJoinOutput {
				paired_device_id: dev,
				device_name: name,
			})
		} else {
			Err(ActionError::Internal(
				"Pairing did not complete".to_string(),
			))
		}
	}

	fn action_kind(&self) -> &'static str {
		"network.pair.join"
	}
}

crate::register_core_action!(PairJoinAction, "network.pair.join");
