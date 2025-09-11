//! Network status query

use super::output::NetworkStatus;
use crate::{context::CoreContext, cqrs::Query};
use anyhow::Result;
use std::sync::Arc;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NetworkStatusQuery;

impl Query for NetworkStatusQuery {
	type Output = NetworkStatus;

	async fn execute(self, context: Arc<CoreContext>) -> Result<Self::Output> {
		let networking = context.get_networking().await;
		if let Some(net) = networking {
			let node_id = net.node_id().to_string();
			let addresses = if let Ok(addr) = net.get_node_addr().await {
				addr
					.direct_addresses()
					.map(|a| a.to_string())
					.collect::<Vec<_>>()
			} else {
				Vec::new()
			};
			let paired = {
				let reg = net.device_registry();
				let guard = reg.read().await;
				guard.get_paired_devices().len()
			};
			let connected = net.get_connected_devices().await.len();
			Ok(NetworkStatus {
				running: true,
				node_id: Some(node_id),
				addresses,
				paired_devices: paired,
				connected_devices: connected,
				version: env!("CARGO_PKG_VERSION").to_string(),
			})
		} else {
			Ok(NetworkStatus {
				running: false,
				node_id: None,
				addresses: Vec::new(),
				paired_devices: 0,
				connected_devices: 0,
				version: env!("CARGO_PKG_VERSION").to_string(),
			})
		}
	}
}

crate::register_query!(NetworkStatusQuery, "network.status");

