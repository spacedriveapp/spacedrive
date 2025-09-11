//! Initialize networking action

use super::{input::NetworkInitInput, output::NetworkInitOutput};
use crate::infra::action::{error::ActionError, CoreAction};
use std::sync::Arc;

pub struct NetworkInitAction;

impl CoreAction for NetworkInitAction {
	type Output = NetworkInitOutput;
	type Input = NetworkInitInput;

	fn from_input(_input: Self::Input) -> std::result::Result<Self, String> { Ok(Self) }

	async fn execute(self, context: Arc<crate::context::CoreContext>) -> std::result::Result<Self::Output, ActionError> {
		// If networking already initialized, just return identifiers
		if let Some(net) = context.get_networking().await {
			return Ok(NetworkInitOutput { device_id: net.device_id(), node_id: Some(net.node_id().to_string()) });
		}

		// Initialize fresh NetworkingService, start it, and register protocols using existing handlers
		let device_manager = context.device_manager.clone();
		let library_key_manager = context.library_key_manager.clone();
		let data_dir = crate::config::default_data_dir().map_err(|e| ActionError::Internal(e.to_string()))?;
		let logger: Arc<dyn crate::service::network::utils::logging::NetworkLogger> = Arc::new(crate::service::network::utils::logging::ConsoleLogger);

		let mut svc = crate::service::network::core::NetworkingService::new(
			device_manager,
			library_key_manager,
			data_dir.clone(),
			logger.clone(),
		).await.map_err(|e| ActionError::Internal(e.to_string()))?;

		// Start event loop and endpoint
		svc.start().await.map_err(|e| ActionError::Internal(e.to_string()))?;

		// Register protocol handlers mirroring Core::register_default_protocols
		let command_sender = svc
			.command_sender()
			.ok_or(ActionError::Internal("NetworkingEventLoop command sender not available".to_string()))?
			.clone();

		let pairing_handler = std::sync::Arc::new(
			crate::service::network::protocol::PairingProtocolHandler::new_with_persistence(
				svc.identity().clone(),
				svc.device_registry(),
				logger.clone(),
				command_sender,
				data_dir,
			),
		);

		// Load sessions (best-effort)
		let _ = pairing_handler.load_persisted_sessions().await;
		crate::service::network::protocol::PairingProtocolHandler::start_state_machine_task(pairing_handler.clone());
		crate::service::network::protocol::PairingProtocolHandler::start_cleanup_task(pairing_handler.clone());

		let messaging_handler = crate::service::network::protocol::MessagingProtocolHandler::new();
		let mut file_transfer_handler = crate::service::network::protocol::FileTransferProtocolHandler::new_default(logger.clone());
		file_transfer_handler.set_device_registry(svc.device_registry());

		{
			let mut reg = svc.protocol_registry().write().await;
			reg.register_handler(pairing_handler).map_err(|e| ActionError::Internal(e.to_string()))?;
			reg.register_handler(std::sync::Arc::new(messaging_handler)).map_err(|e| ActionError::Internal(e.to_string()))?;
			reg.register_handler(std::sync::Arc::new(file_transfer_handler)).map_err(|e| ActionError::Internal(e.to_string()))?;
		}

		// Make available to context
		let device_id = svc.device_id();
		let node_id = svc.node_id().to_string();
		context.set_networking(Arc::new(svc)).await;

		Ok(NetworkInitOutput { device_id, node_id: Some(node_id) })
	}

	fn action_kind(&self) -> &'static str { "network.init" }
}

crate::register_core_action!(NetworkInitAction, "network.init");

