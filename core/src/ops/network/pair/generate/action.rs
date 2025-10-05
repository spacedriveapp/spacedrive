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
			.start_pairing_as_initiator()
			.await
			.map_err(|e| ActionError::Internal(e.to_string()))?;
		// Derive session_id from code (using PairingCode parser)
		let pairing_code =
			crate::service::network::protocol::pairing::PairingCode::from_string(&code)
				.map_err(|e| ActionError::Internal(e.to_string()))?;
		let session_id = pairing_code.session_id();
		Ok(PairGenerateOutput {
			code,
			session_id,
			expires_at: Utc::now() + chrono::Duration::seconds(expires_in as i64),
		})
	}

	fn action_kind(&self) -> &'static str {
		"network.pair.generate"
	}
}

crate::register_core_action!(PairGenerateAction, "network.pair.generate");
