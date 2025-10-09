//! Network status query

use super::output::NetworkStatus;
use crate::infra::query::QueryResult;
use crate::{context::CoreContext, infra::query::CoreQuery};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct NetworkStatusQueryInput;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct NetworkStatusQuery;

impl CoreQuery for NetworkStatusQuery {
	type Input = NetworkStatusQueryInput;
	type Output = NetworkStatus;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self)
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let networking = context.get_networking().await;
		if let Some(net) = networking {
			let node_id = net.node_id().to_string();
			let addresses = if let Ok(Some(addr)) = net.get_node_addr() {
				addr.direct_addresses()
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

crate::register_core_query!(NetworkStatusQuery, "network.status");
