//! List devices query

use super::output::DeviceInfoLite;
use crate::{context::CoreContext, cqrs::CoreQuery};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ListDevicesInput {
	/// Only include paired devices
	pub paired_only: bool,
	/// Only include currently connected devices
	pub connected_only: bool,
}

#[derive(Debug, Clone)]
pub struct ListDevicesQuery {
	input: ListDevicesInput,
}

impl ListDevicesQuery {
	pub fn paired() -> Self {
		Self {
			input: ListDevicesInput {
				paired_only: true,
				connected_only: false,
			},
		}
	}
	pub fn connected() -> Self {
		Self {
			input: ListDevicesInput {
				paired_only: false,
				connected_only: true,
			},
		}
	}
	pub fn all() -> Self {
		Self {
			input: ListDevicesInput {
				paired_only: false,
				connected_only: false,
			},
		}
	}
}

impl CoreQuery for ListDevicesQuery {
	type Input = ListDevicesInput;

	fn from_input(input: Self::Input) -> Result<Self> {
		Ok(Self { input })
	}

	type Output = Vec<DeviceInfoLite>;

	async fn execute(self, context: Arc<CoreContext>) -> Result<Self::Output> {
		let mut out: Vec<DeviceInfoLite> = Vec::new();
		if let Some(net) = context.get_networking().await {
			let reg = net.device_registry();
			let guard = reg.read().await;
			let mut devices = if self.input.connected_only {
				guard.get_connected_devices()
			} else if self.input.paired_only {
				guard.get_paired_devices()
			} else {
				let mut v = guard.get_paired_devices();
				let mut c = guard.get_connected_devices();
				v.append(&mut c);
				// de-dup by device_id
				v.sort_by_key(|d| d.device_id);
				v.dedup_by_key(|d| d.device_id);
				v
			};

			for d in devices.drain(..) {
				out.push(DeviceInfoLite {
					id: d.device_id,
					name: d.device_name,
					os_version: d.os_version,
					app_version: d.app_version,
					is_connected: matches!(
						guard.get_device_state(d.device_id),
						Some(crate::service::network::device::DeviceState::Connected { .. })
					),
					last_seen: d.last_seen,
				});
			}
		}
		Ok(out)
	}
}

crate::register_core_query!(ListDevicesQuery, "network.devices");
