use super::{output::IndexResetOutput, query::IndexResetInput};
use crate::infra::action::{error::ActionError, CoreAction};
use std::sync::Arc;
use tracing::info;

pub struct IndexResetAction {
	input: IndexResetInput,
}

impl CoreAction for IndexResetAction {
	type Output = IndexResetOutput;
	type Input = IndexResetInput;

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

		info!("Resetting the volume index");

		let cache = context.volume_index();
		let cleared_paths = cache.clear_all().await;

		info!(
			"Volume index reset complete. Cleared {} paths",
			cleared_paths
		);

		Ok(IndexResetOutput {
			cleared_paths,
			message: format!("Volume index reset. Cleared {} paths", cleared_paths),
		})
	}

	fn action_kind(&self) -> &'static str {
		"core.index_reset"
	}
}

crate::register_core_action!(IndexResetAction, "core.index_reset");
