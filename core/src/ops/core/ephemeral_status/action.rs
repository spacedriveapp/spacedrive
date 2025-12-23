use super::{output::EphemeralCacheResetOutput, query::EphemeralCacheResetInput};
use crate::infra::action::{error::ActionError, CoreAction};
use std::sync::Arc;
use tracing::info;

pub struct EphemeralCacheResetAction {
	input: EphemeralCacheResetInput,
}

impl CoreAction for EphemeralCacheResetAction {
	type Output = EphemeralCacheResetOutput;
	type Input = EphemeralCacheResetInput;

	fn from_input(input: Self::Input) -> std::result::Result<Self, String> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<crate::context::CoreContext>,
	) -> std::result::Result<Self::Output, ActionError> {
		if !self.input.confirm {
			return Err(ActionError::InvalidInput(
				"Reset must be confirmed".to_string(),
			));
		}

		info!("Resetting ephemeral cache");

		let cache = context.ephemeral_cache();
		let cleared_paths = cache.clear_all().await;

		info!(
			"Ephemeral cache reset complete. Cleared {} paths",
			cleared_paths
		);

		Ok(EphemeralCacheResetOutput {
			cleared_paths,
			message: format!("Ephemeral cache reset. Cleared {} paths", cleared_paths),
		})
	}

	fn action_kind(&self) -> &'static str {
		"core.ephemeral_reset"
	}
}

crate::register_core_action!(EphemeralCacheResetAction, "core.ephemeral_reset");
