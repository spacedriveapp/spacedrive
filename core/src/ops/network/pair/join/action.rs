use super::{input::PairJoinInput, output::PairJoinOutput};
use crate::infra::action::{error::ActionError, CoreAction};
use std::sync::Arc;

pub struct PairJoinAction {
	pub code: String,
}

impl CoreAction for PairJoinAction {
	type Output = PairJoinOutput;
	type Input = PairJoinInput;

	fn from_input(input: Self::Input) -> std::result::Result<Self, String> {
		Ok(Self { code: input.code })
	}

	async fn execute(
		self,
		context: Arc<crate::context::CoreContext>,
	) -> std::result::Result<Self::Output, ActionError> {
		let net = context
			.get_networking()
			.await
			.ok_or_else(|| ActionError::Internal("Networking not initialized".to_string()))?;
		net.start_pairing_as_joiner(&self.code)
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
