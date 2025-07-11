//! Background services management

use crate::{
	context::CoreContext, infrastructure::events::EventBus,
	keys::library_key_manager::LibraryKeyManager,
};
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

pub mod device;
pub mod file_sharing;
pub mod location_watcher;
pub mod networking;

use device::DeviceService;
use file_sharing::FileSharingService;
use location_watcher::{LocationWatcher, LocationWatcherConfig};
use networking::NetworkingService;

/// Container for all background services
pub struct Services {
	/// File system watcher for locations
	pub location_watcher: Arc<LocationWatcher>,
	/// File sharing service
	pub file_sharing: Arc<FileSharingService>,
	/// Device management service
	pub device: Arc<DeviceService>,
	/// Networking service for device connections
	pub networking: Option<Arc<NetworkingService>>,
	/// Library key manager
	pub library_key_manager: Arc<LibraryKeyManager>,
	/// Shared context for all services
	context: Arc<CoreContext>,
}

impl Services {
	/// Create new services container with context
	pub fn new(context: Arc<CoreContext>) -> Self {
		info!("Initializing background services");

		let location_watcher_config = LocationWatcherConfig::default();
		let location_watcher = Arc::new(LocationWatcher::new(
			location_watcher_config,
			context.events.clone(),
		));
		let file_sharing = Arc::new(FileSharingService::new(context.clone()));
		let device = Arc::new(DeviceService::new(context.clone()));
		let library_key_manager = context.library_key_manager.clone();

		Self {
			location_watcher,
			file_sharing,
			device,
			networking: None, // Initialized separately when needed
			library_key_manager,
			context,
		}
	}

	/// Get the shared context
	pub fn context(&self) -> Arc<CoreContext> {
		self.context.clone()
	}

	/// Start all services
	pub async fn start_all(&self) -> Result<()> {
		info!("Starting all background services");

		self.location_watcher.start().await?;

		// Networking service is already started during initialization

		// TODO: Start other services
		// self.jobs.start().await?;
		// self.thumbnails.start().await?;

		Ok(())
	}

	/// Stop all services gracefully
	pub async fn stop_all(&self) -> Result<()> {
		info!("Stopping all background services");

		self.location_watcher.stop().await?;

		// Stop networking service if initialized
		if let Some(networking) = &self.networking {
			networking
				.shutdown()
				.await
				.map_err(|e| anyhow::anyhow!("Failed to stop networking: {}", e))?;
		}

		Ok(())
	}

	/// Initialize networking service
	pub async fn init_networking(
		&mut self,
		device_manager: std::sync::Arc<crate::device::DeviceManager>,
		library_key_manager: std::sync::Arc<crate::keys::library_key_manager::LibraryKeyManager>,
		data_dir: impl AsRef<std::path::Path>,
	) -> Result<()> {
		use crate::services::networking::{NetworkingService, utils::logging::ConsoleLogger};

		info!("Initializing networking service");
		let logger = std::sync::Arc::new(ConsoleLogger);
		let networking_service =
			NetworkingService::new(device_manager, library_key_manager, data_dir, logger)
				.await
				.map_err(|e| anyhow::anyhow!("Failed to create networking service: {}", e))?;

		self.networking = Some(Arc::new(networking_service));
		Ok(())
	}

	/// Start networking service after initialization
	pub async fn start_networking(&self) -> Result<()> {
		if let Some(networking) = &self.networking {
			// Create a temporary mutable reference to start the service
			// This is safe because start() is only called once during initialization
			let networking_ptr =
				Arc::as_ptr(networking) as *mut crate::services::networking::NetworkingService;
			unsafe {
				(*networking_ptr)
					.start()
					.await
					.map_err(|e| anyhow::anyhow!("Failed to start networking service: {}", e))?;
			}
		}
		Ok(())
	}

	/// Get networking service if initialized
	pub fn networking(&self) -> Option<Arc<NetworkingService>> {
		self.networking.clone()
	}
}

/// Trait for background services
#[async_trait::async_trait]
pub trait Service: Send + Sync {
	/// Start the service
	async fn start(&self) -> Result<()>;

	/// Stop the service gracefully
	async fn stop(&self) -> Result<()>;

	/// Check if the service is running
	fn is_running(&self) -> bool;

	/// Get service name for logging
	fn name(&self) -> &'static str;
}
