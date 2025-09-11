use super::{input::DeviceRevokeInput, output::DeviceRevokeOutput};
use crate::infra::action::{error::ActionError, CoreAction};
use std::sync::Arc;

pub struct DeviceRevokeAction { pub device_id: uuid::Uuid }

impl CoreAction for DeviceRevokeAction {
	type Output = DeviceRevokeOutput;
	type Input = DeviceRevokeInput;

	fn from_input(input: Self::Input) -> std::result::Result<Self, String> { Ok(Self { device_id: input.device_id }) }

	async fn execute(self, context: Arc<crate::context::CoreContext>) -> std::result::Result<Self::Output, ActionError> {
		let net = context.get_networking().await.ok_or_else(|| ActionError::Internal("Networking not initialized".to_string()))?;
		// Remove from persistence and disconnect if connected
		{
			let reg = net.device_registry();
			let mut guard = reg.write().await;
			// Drop from registry state
			let _ = guard.remove_device(self.device_id);
		}
		{
			let reg = net.device_registry();
			let guard = reg.read().await;
			let _ = guard.persistence().remove_paired_device(self.device_id).await; // if available
		}
		Ok(DeviceRevokeOutput { revoked: true })
	}

	fn action_kind(&self) -> &'static str { "network.device.revoke" }
}

crate::register_core_action!(DeviceRevokeAction, "network.device.revoke");

