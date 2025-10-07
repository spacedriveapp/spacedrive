//! Query for listing paired devices

use super::output::{ListPairedDevicesOutput, PairedDeviceInfo};
use crate::{
	context::CoreContext,
	infra::query::{CoreQuery, QueryError, QueryResult},
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ListPairedDevicesInput {
	/// Whether to include only connected devices
	#[serde(default)]
	pub connected_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ListPairedDevicesQuery {
	connected_only: bool,
}

impl CoreQuery for ListPairedDevicesQuery {
	type Input = ListPairedDevicesInput;
	type Output = ListPairedDevicesOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self {
			connected_only: input.connected_only,
		})
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		_session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		// Get networking service
		let networking = context
			.get_networking()
			.await
			.ok_or_else(|| QueryError::Internal("Networking not initialized".to_string()))?;

		let device_registry = networking.device_registry();
		let registry = device_registry.read().await;

		let all_devices = registry.get_all_devices();
		let mut devices = Vec::new();
		let mut connected_count = 0;

		for (device_id, state) in all_devices {
			use crate::service::network::device::DeviceState;

			let (device_info, is_connected) = match state {
				DeviceState::Paired { info, .. } => (Some(info), false),
				DeviceState::Connected { info, .. } => {
					connected_count += 1;
					(Some(info), true)
				}
				DeviceState::Disconnected { info, .. } => (Some(info), false),
				_ => (None, false),
			};

			// Skip if we only want connected devices and this one isn't connected
			if self.connected_only && !is_connected {
				continue;
			}

			if let Some(info) = device_info {
				let device_type_str = format!("{:?}", info.device_type);

				devices.push(PairedDeviceInfo {
					id: info.device_id,
					name: info.device_name.clone(),
					device_type: device_type_str,
					os_version: info.os_version.clone(),
					app_version: info.app_version.clone(),
					is_connected,
					last_seen: info.last_seen,
				});
			}
		}

		let total = devices.len();

		Ok(ListPairedDevicesOutput {
			devices,
			total,
			connected: connected_count,
		})
	}
}

crate::register_core_query!(ListPairedDevicesQuery, "network.devices.list");
