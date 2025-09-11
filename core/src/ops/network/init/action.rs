//! Initialize networking action

use super::{input::NetworkInitInput, output::NetworkInitOutput};
use crate::infra::action::{error::ActionError, CoreAction};
use anyhow::Result;
use std::sync::Arc;

pub struct NetworkInitAction {
	password: Option<String>,
}

impl CoreAction for NetworkInitAction {
	type Output = NetworkInitOutput;
	type Input = NetworkInitInput;

	fn from_input(input: Self::Input) -> std::result::Result<Self, String> {
		Ok(Self { password: input.password })
	}

	async fn execute(self, context: Arc<crate::context::CoreContext>) -> std::result::Result<Self::Output, ActionError> {
		// Initialize networking service if not present
		let maybe_net = context.get_networking().await;
		if maybe_net.is_none() {
			let device_manager = context.device_manager.clone();
			let library_key_manager = context.library_key_manager.clone();
			let data_dir = std::env::var("SPACEDRIVE_DATA_DIR").ok().map(std::path::PathBuf::from).unwrap_or_else(|| std::path::PathBuf::from(".sd"));
			let logger: Arc<dyn crate::service::network::utils::logging::NetworkLogger> = Arc::new(crate::service::network::utils::logging::StdoutLogger);
			let mut svc = crate::service::network::core::NetworkingService::new(
				device_manager,
				library_key_manager,
				data_dir,
				logger,
			).await.map_err(|e| ActionError::Internal(e.to_string()))?;

			// Register pairing protocol
			{
				let mut reg = svc.protocol_registry().write().await;
				reg.register_pairing();
			}

			context.set_networking(Arc::new(svc)).await;
		}

		let net = context.get_networking().await.unwrap();
		Ok(NetworkInitOutput { device_id: net.device_id(), node_id: None })
	}

	fn action_kind(&self) -> &'static str { "network.init" }
}

crate::register_core_action!(NetworkInitAction, "network.init");

