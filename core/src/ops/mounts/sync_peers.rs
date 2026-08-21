//! Replicate source indexes from every connected paired device, making them
//! browsable (and streamable) through the mounts share.

use crate::{
	infra::action::{error::ActionError, CoreAction},
	service::mounts::peer,
	service::network::device::DeviceState,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Type, Default)]
pub struct MountsSyncPeersInput {}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct MountsSyncPeersOutput {
	pub devices: u32,
	pub sources: u32,
	pub message: String,
}

pub struct MountsSyncPeersAction {
	#[allow(dead_code)]
	input: MountsSyncPeersInput,
}

impl CoreAction for MountsSyncPeersAction {
	type Input = MountsSyncPeersInput;
	type Output = MountsSyncPeersOutput;

	fn from_input(input: Self::Input) -> std::result::Result<Self, String> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<crate::context::CoreContext>,
	) -> std::result::Result<Self::Output, ActionError> {
		let networking =
			context.networking.read().await.clone().ok_or_else(|| {
				ActionError::Internal("networking service not available".to_string())
			})?;

		let connected: Vec<(uuid::Uuid, String)> = {
			let registry = networking.device_registry();
			let registry = registry.read().await;
			registry
				.get_all_devices()
				.into_iter()
				.filter_map(|(device_id, state)| match state {
					DeviceState::Connected { info, .. } => Some((device_id, info.device_name)),
					_ => None,
				})
				.collect()
		};

		let mut devices = 0u32;
		let mut sources = 0u32;
		for (device_id, name) in connected {
			match peer::sync_device(&context, device_id, name.clone()).await {
				Ok(count) => {
					devices += 1;
					sources += count as u32;
				}
				Err(err) => {
					tracing::warn!("peer sync with {name} ({device_id}) failed: {err}");
				}
			}
		}

		Ok(MountsSyncPeersOutput {
			devices,
			sources,
			message: format!("replicated {sources} source(s) from {devices} device(s)"),
		})
	}

	fn action_kind(&self) -> &'static str {
		"mounts.sync_peers"
	}
}

crate::register_core_action!(MountsSyncPeersAction, "mounts.sync_peers");
