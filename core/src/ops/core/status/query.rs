//! Core status query (modular)

use super::output::*;
use crate::{
	context::CoreContext,
	cqrs::{CoreQuery, Query},
	service::Service,
};
use anyhow::Result;
use chrono::Utc;

use std::sync::Arc;

#[derive(Debug)]
pub struct CoreStatusQuery;

impl CoreQuery for CoreStatusQuery {
	type Input = ();
	type Output = CoreStatus;

	fn from_input(input: Self::Input) -> Result<Self> {
		Ok(Self)
	}

	async fn execute(self, context: Arc<CoreContext>, session: crate::infra::api::SessionContext) -> Result<Self::Output> {
		// Get basic library information
		let library_manager = context.libraries().await;
		let libs = library_manager.list().await;
		let active_library = library_manager.get_active_library().await;

		// Get device information
		let device_config = context.device_manager.config()?;
		let device_info = DeviceInfo {
			id: device_config.id,
			name: device_config.name,
			os: device_config.os,
			hardware_model: device_config.hardware_model,
			created_at: device_config.created_at,
		};

		// Collect detailed library information
		let mut libraries = Vec::new();
		for lib in &libs {
			let is_active = active_library
				.as_ref()
				.map(|al| al.id() == lib.id())
				.unwrap_or(false);

			// Get location count for this library
			let location_count = match crate::location::list_locations(lib.clone()).await {
				Ok(locations) => locations.len(),
				Err(_) => 0,
			};

			// TODO: Get total entries count from database
			// This would require a database query to count entries in this library
			let total_entries = None;

			libraries.push(LibraryInfo {
				id: lib.id(),
				name: lib.name().await,
				is_active,
				location_count,
				total_entries,
				last_sync: None, // TODO: Get last sync time from library metadata
			});
		}

		// Get service status from the services in context
		// Note: We need to access the services through the Core instance
		// For now, we'll provide basic status information
		let services = ServiceStatus {
			location_watcher: ServiceState {
				running: true, // TODO: Get actual status from service
				details: Some("Monitoring file system changes".to_string()),
			},
			networking: ServiceState {
				running: context.get_networking().await.is_some(),
				details: if context.get_networking().await.is_some() {
					Some("P2P networking enabled".to_string())
				} else {
					Some("P2P networking disabled".to_string())
				},
			},
			volume_monitor: ServiceState {
				running: true, // TODO: Get actual status
				details: Some("Monitoring volume changes".to_string()),
			},
			file_sharing: ServiceState {
				running: true, // TODO: Get actual status
				details: Some("File sharing service active".to_string()),
			},
		};

		// Get network status and paired devices
		let network_status = if let Some(networking) = context.get_networking().await {
			// Get paired devices from networking service
			let paired_devices = Vec::new(); // TODO: Implement getting paired devices

			NetworkStatus {
				enabled: true,
				node_id: Some(networking.node_id().to_string()),
				paired_devices,
				active_connections: 0,   // TODO: Get actual connection count
				discovery_enabled: true, // TODO: Get actual discovery status
			}
		} else {
			NetworkStatus {
				enabled: false,
				node_id: None,
				paired_devices: Vec::new(),
				active_connections: 0,
				discovery_enabled: false,
			}
		};

		// Get current library name from the active library
		let current_library = if let Some(active_lib) = &active_library {
			Some(active_lib.name().await)
		} else {
			None
		};

		// System information
		let system = SystemInfo {
			uptime: None, // TODO: Calculate uptime from service start time
			data_directory: std::env::var("SPACEDRIVE_DATA_DIR")
				.unwrap_or_else(|_| "default".to_string()),
			instance_name: std::env::var("SPACEDRIVE_INSTANCE").ok(),
			current_library,
		};

		Ok(CoreStatus {
			version: env!("CARGO_PKG_VERSION").to_string(),
			built_at: env!("BUILD_TIMESTAMP").to_string(),
			library_count: libs.len(),
			device_info,
			libraries,
			services,
			network: network_status,
			system,
		})
	}
}

crate::register_core_query!(CoreStatusQuery, "core.status");
