use super::{input::PairGenerateInput, output::PairGenerateOutput};
use crate::infra::action::{error::ActionError, CoreAction};
use chrono::Utc;
use std::sync::Arc;

pub struct PairGenerateAction {
	pub auto_accept: bool,
}

impl CoreAction for PairGenerateAction {
	type Output = PairGenerateOutput;
	type Input = PairGenerateInput;

	fn from_input(input: Self::Input) -> std::result::Result<Self, String> {
		Ok(Self {
			auto_accept: input.auto_accept,
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
		let (code, expires_in) = net
			.start_pairing_as_initiator(false)
			.await
			.map_err(|e| ActionError::Internal(e.to_string()))?;

		// Get the full PairingCode object with NodeId and relay info
		let pairing_code = net
			.get_pairing_code_for_current_session()
			.await
			.map_err(|e| ActionError::Internal(e.to_string()))?
			.ok_or_else(|| ActionError::Internal("No pairing code found".to_string()))?;

		let session_id = pairing_code.session_id();
		let qr_json = pairing_code.to_qr_json();

		Ok(PairGenerateOutput {
			code,
			session_id,
			expires_at: Utc::now() + chrono::Duration::seconds(expires_in as i64),
			qr_json,
		})
	}

	fn action_kind(&self) -> &'static str {
		"network.pair.generate"
	}
}

crate::register_core_action!(PairGenerateAction, "network.pair.generate");
