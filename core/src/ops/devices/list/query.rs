//! List devices from library database query

use crate::{
	context::CoreContext,
	device::get_current_device_id,
	domain::Device,
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
					// Set ephemeral fields
					device.is_current = device.id == current_device_id;
					device.is_paired = false; // DB devices are registered, not just paired
					device.is_connected = false; // Will be updated if also in network registry
					result.push(device);
				}
				Err(e) => {
					tracing::warn!("Failed to convert device model: {}", e);
				}
			}
		}

		// If show_paired is true, also fetch paired network devices
		if self.input.show_paired {
			// Get networking service
			if let Some(networking) = context.get_networking().await {
				let device_registry = networking.device_registry();
				let registry = device_registry.read().await;
				let all_devices = registry.get_all_devices();

				for (device_id, state) in all_devices {
					// Check if this device is already in the library results
					if let Some(existing) = result.iter_mut().find(|d| d.id == device_id) {
						// Update connection status for library device that's also paired
						use crate::service::network::device::DeviceState;
						if matches!(state, DeviceState::Connected { .. }) {
							existing.is_connected = true;
							existing.is_online = true;
						}
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

						// Convert network DeviceInfo to domain Device
						let device = Device::from_network_info(&info, is_connected);
						result.push(device);
					}
				}
			}
		}

		Ok(result)
	}
}

crate::register_library_query!(ListLibraryDevicesQuery, "devices.list");
