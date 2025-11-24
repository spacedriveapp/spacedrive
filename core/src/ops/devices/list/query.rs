//! List devices from library database query

use super::output::LibraryDeviceInfo;
use crate::{
	context::CoreContext,
	device::get_current_device_id,
	infra::query::{LibraryQuery, QueryError, QueryResult},
};
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
	type Output = Vec<LibraryDeviceInfo>;

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
		let devices = query
			.order_by_desc(crate::infra::db::entities::device::Column::LastSeenAt)
			.all(db)
			.await?;

		// Convert to output format
		let mut result = Vec::new();
		for device in devices {
			// Parse JSON fields if details are requested
			let network_addresses = if self.input.include_details {
				serde_json::from_value(device.network_addresses.clone()).unwrap_or_default()
			} else {
				Vec::new()
			};

			let capabilities = if self.input.include_details {
				Some(device.capabilities.clone())
			} else {
				None
			};

			result.push(LibraryDeviceInfo {
				id: device.uuid,
				name: device.name,
				os: device.os,
				os_version: device.os_version,
				hardware_model: device.hardware_model,
				is_online: device.is_online,
				last_seen_at: device.last_seen_at,
				created_at: device.created_at,
				updated_at: device.updated_at,
				is_current: device.uuid == current_device_id,
				network_addresses,
				capabilities,
				is_paired: false,
				is_connected: false,
			});
		}

		// If show_paired is true, also fetch paired network devices
		if self.input.show_paired {
			// Get networking service
			if let Some(networking) = context.get_networking().await {
				let device_registry = networking.device_registry();
				let registry = device_registry.read().await;
				let all_devices = registry.get_all_devices();

				for (device_id, state) in all_devices {
					// Skip if this device is already in the library
					if result.iter().any(|d| d.id == device_id) {
						continue;
					}

					use crate::service::network::device::DeviceState;

					let (device_info, is_connected) = match state {
						DeviceState::Paired { info, .. } => (Some(info), false),
						DeviceState::Connected { info, .. } => (Some(info), true),
						DeviceState::Disconnected { info, .. } => (Some(info), false),
						_ => (None, false),
					};

					if let Some(info) = device_info {
						// Filter by online status if requested
						if !self.input.include_offline && !is_connected {
							continue;
						}

						result.push(LibraryDeviceInfo {
							id: device_id,
							name: info.device_name.clone(),
							os: format!("{:?}", info.device_type),
							os_version: Some(info.os_version.clone()),
							hardware_model: None,
							is_online: is_connected,
							last_seen_at: info.last_seen,
							created_at: info.last_seen, // Use last_seen as fallback
							updated_at: info.last_seen,
							is_current: false,
							network_addresses: Vec::new(),
							capabilities: None,
							is_paired: true,
							is_connected,
						});
					}
				}
			}
		}

		Ok(result)
	}
}

crate::register_library_query!(ListLibraryDevicesQuery, "devices.list");
