//! List devices from library database query

use crate::{
	context::CoreContext,
	device::get_current_device_id,
	domain::Device,
	infra::query::{LibraryQuery, QueryError, QueryResult},
};
use iroh::Watcher;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

/// Input for listing devices from library database
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ListLibraryDevicesInput {
	/// Whether to include offline devices (default: true)
	pub include_offline: bool,

	/// Whether to include detailed capabilities and sync leadership info (default: false)
	pub include_details: bool,

	/// Whether to also include paired network devices (default: false)
	#[serde(default)]
	pub show_paired: bool,
}

/// Query to list all devices from the library database
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ListLibraryDevicesQuery {
	input: ListLibraryDevicesInput,
}

impl ListLibraryDevicesQuery {
	/// Create a basic device list query
	pub fn basic() -> Self {
		Self {
			input: ListLibraryDevicesInput {
				include_offline: true,
				include_details: false,
				show_paired: false,
			},
		}
	}

	/// Create a detailed device list query
	pub fn detailed() -> Self {
		Self {
			input: ListLibraryDevicesInput {
				include_offline: true,
				include_details: true,
				show_paired: false,
			},
		}
	}

	/// Create a query for online devices only
	pub fn online_only() -> Self {
		Self {
			input: ListLibraryDevicesInput {
				include_offline: false,
				include_details: false,
				show_paired: false,
			},
		}
	}
}

impl LibraryQuery for ListLibraryDevicesQuery {
	type Input = ListLibraryDevicesInput;
	type Output = Vec<Device>;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		// Get the current library from session
		let library_id = session
			.current_library_id
			.ok_or_else(|| QueryError::Internal("No library in session".to_string()))?;

		let library = context
			.libraries()
			.await
			.get_library(library_id)
			.await
			.ok_or_else(|| QueryError::LibraryNotFound(library_id))?;

		// Get database connection
		let db = library.db().conn();

		// Get current device ID for comparison
		let current_device_id = get_current_device_id();

		// Build query to fetch devices from database
		let mut query = crate::infra::db::entities::device::Entity::find();

		// Filter out offline devices if requested
		if !self.input.include_offline {
			query = query.filter(crate::infra::db::entities::device::Column::IsOnline.eq(true));
		}

		// Execute query
		let device_models = query
			.order_by_desc(crate::infra::db::entities::device::Column::LastSeenAt)
			.all(db)
			.await?;

		// Convert to Device domain model
		let mut result = Vec::new();
		for model in device_models {
			match Device::try_from(model) {
				Ok(mut device) => {
					// Set ephemeral fields (defaults - will be updated when merging with network state)
					device.is_current = device.id == current_device_id;
					device.is_paired = false; // Updated below if device is also in network registry
					device.is_connected = false; // Updated below if device is connected via network

					// For remote devices, set is_online based on network connection (will be updated below)
					// For current device, it's always online
					if device.is_current {
						device.is_online = true;
					} else {
						device.is_online = false; // Will be set to true if connected via network
					}

					result.push(device);
				}
				Err(e) => {
					tracing::warn!("Failed to convert device model: {}", e);
				}
			}
		}

		// Always check network registry to update connection status for database devices
		// and optionally add paired-only devices
		if let Some(networking) = context.get_networking().await {
			// Hardware each paired device published about itself, which the
			// pairing handshake does not carry.
			let published = crate::service::mounts::peer::published_facts().await;

			let device_registry = networking.device_registry();
			let registry = device_registry.read().await;
			let all_devices = registry.get_all_devices();

			// Get Iroh endpoint for verifying actual connection status
			// This is the source of truth, not the cached DeviceState
			let endpoint = networking.endpoint();

			for (device_id, state) in all_devices {
				use crate::service::network::device::DeviceState;

				// Query Iroh directly for actual connection status and method
				let (is_actually_connected, connection_method) = if let Some(ep) = endpoint {
					// Get node ID for this device
					if let Some(node_id) = registry.get_node_id_for_device(device_id) {
						// Use conn_type() API (replaces remote_info() removed in v0.93+)
						if let Some(mut conn_type_watcher) = ep.conn_type(node_id) {
							// Get current connection type from watcher using the Watcher trait's get() method
							let conn_type = conn_type_watcher.get();
							// The watcher's path info can read None between
							// requests even while the registry holds a live
							// connection; either source saying connected is
							// enough.
							let is_connected =
								!matches!(conn_type, iroh::endpoint::ConnectionType::None)
									|| matches!(state, DeviceState::Connected { .. });
							let conn_method =
								crate::domain::device::ConnectionMethod::from_iroh_connection_type(
									conn_type,
								);
							(is_connected, conn_method)
						} else {
							// No address information exists for this endpoint (never connected)
							(false, None)
						}
					} else {
						(false, None)
					}
				} else {
					// No endpoint available, fall back to cached state
					let is_connected = matches!(state, DeviceState::Connected { .. });
					(is_connected, None)
				};

				// Check if this device is already in the library results
				if let Some(existing) = result.iter_mut().find(|d| d.id == device_id) {
					// Update pairing/connection status for library device that's also in network registry
					match state {
						DeviceState::Paired { .. }
						| DeviceState::Connected { .. }
						| DeviceState::Disconnected { .. } => {
							existing.is_paired = true;
						}
						_ => {}
					}

					// Always update online/connected status based on current network state
					// (database is_online column can be stale for remote devices)
					// TODO: remove that column imo
					existing.is_connected = is_actually_connected;
					existing.is_online = is_actually_connected;
					existing.connection_method = connection_method;
					if let Some(observed) = published.get(&device_id) {
						observed.facts.hardware.apply_to(existing);
					}

					continue;
				}

				// Only add paired-only devices (not in database) if show_paired is true
				if !self.input.show_paired {
					continue;
				}

				let device_info = match state {
					DeviceState::Paired { info, .. } => Some(info),
					DeviceState::Connected { info, .. } => Some(info),
					DeviceState::Disconnected { info, .. } => Some(info),
					_ => None,
				};

				if let Some(info) = device_info {
					// Filter by online status if requested
					if !self.input.include_offline && !is_actually_connected {
						continue;
					}

					// Convert network DeviceInfo to domain Device
					let mut device =
						Device::from_network_info(&info, is_actually_connected, connection_method);
					if let Some(observed) = published.get(&device_id) {
						observed.facts.hardware.apply_to(&mut device);
					}
					result.push(device);
				}
			}
		}

		Ok(result)
	}
}

crate::register_library_query!(ListLibraryDevicesQuery, "devices.list");
