//! Query for discovering libraries on a remote paired device

use super::output::{DiscoverRemoteLibrariesOutput, RemoteLibraryInfo};
use crate::{
	context::CoreContext,
	infra::query::{CoreQuery, QueryError, QueryResult},
	library::config::LibraryStatistics,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DiscoverRemoteLibrariesInput {
	/// Device ID to query for libraries
	pub device_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DiscoverRemoteLibrariesQuery {
	device_id: Uuid,
}

impl CoreQuery for DiscoverRemoteLibrariesQuery {
	type Input = DiscoverRemoteLibrariesInput;
	type Output = DiscoverRemoteLibrariesOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self {
			device_id: input.device_id,
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

		// Verify device is paired
		let device_registry = networking.device_registry();
		let registry = device_registry.read().await;

		let device_state = registry
			.get_device_state(self.device_id)
			.ok_or_else(|| QueryError::Internal(format!("Device not found: {}", self.device_id)))?;

		// Check if device is paired (Paired, Connected, or Disconnected are all valid)
		let device_info = match device_state {
			crate::service::network::device::DeviceState::Paired { info, .. } => info.clone(),
			crate::service::network::device::DeviceState::Connected { info, .. } => info.clone(),
			crate::service::network::device::DeviceState::Disconnected { info, .. } => info.clone(),
			_ => {
				return Err(QueryError::Internal(format!(
					"Device {} is not paired. Complete pairing first.",
					self.device_id
				)));
			}
		};

		// Check if device is actually online according to Iroh (not cached state)
		let endpoint = networking
			.endpoint()
			.ok_or_else(|| QueryError::Internal("Network endpoint not initialized".to_string()))?;

		let is_online = registry.is_node_connected(endpoint, self.device_id);

		drop(registry);

		// If device is not online, return empty list
		if !is_online {
			return Ok(DiscoverRemoteLibrariesOutput {
				device_id: self.device_id,
				device_name: device_info.device_name.clone(),
				libraries: vec![],
				is_online: false,
			});
		}

		// Send library discovery request to remote device
		use crate::service::network::protocol::library_messages::LibraryMessage;

		let request = LibraryMessage::DiscoveryRequest {
			request_id: Uuid::new_v4(),
		};

		let response = networking
			.send_library_request(self.device_id, request)
			.await
			.map_err(|e| {
				QueryError::Internal(format!("Failed to send library discovery request: {}", e))
			})?;

		// Parse response
		match response {
			LibraryMessage::DiscoveryResponse {
				request_id: _,
				libraries,
			} => {
				let remote_libraries = libraries
					.into_iter()
					.map(|lib| RemoteLibraryInfo {
						id: lib.id,
						name: lib.name,
						description: lib.description,
						created_at: lib.created_at,
						statistics: LibraryStatistics {
							total_files: lib.total_entries,
							total_size: lib.total_size_bytes,
							location_count: lib.total_locations as u32,
							tag_count: 0, // Not available from network protocol
							thumbnail_count: 0, // Not available from network protocol
							database_size: 0, // Not available from network protocol
							last_indexed: None, // Not available from network protocol
							updated_at: chrono::Utc::now(), // Current time
						},
					})
					.collect();

				Ok(DiscoverRemoteLibrariesOutput {
					device_id: self.device_id,
					device_name: device_info.device_name,
					libraries: remote_libraries,
					is_online: true,
				})
			}
			_ => Err(QueryError::Internal(
				"Unexpected response from device".to_string(),
			)),
		}
	}
}

crate::register_core_query!(DiscoverRemoteLibrariesQuery, "network.sync_setup.discover");
