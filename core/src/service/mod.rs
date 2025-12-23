//! Background services management

use crate::{
	context::CoreContext, crypto::key_manager::KeyManager, infra::event::EventBus,
	service::session::SessionStateService,
};
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

pub mod device;
pub mod file_sharing;
pub mod file_sync;
pub mod network;
pub mod session;
pub mod sidecar_manager;
pub mod statistics_listener;
pub mod sync;
pub mod volume_monitor;
pub mod watcher;
// NOTE: watcher_old/ is kept as reference during migration but not compiled

use device::DeviceService;
use file_sharing::FileSharingService;
use network::NetworkingService;
use sidecar_manager::SidecarManager;
use statistics_listener::StatisticsListenerService;
use volume_monitor::{VolumeMonitorConfig, VolumeMonitorService};
use watcher::{FsWatcherService, FsWatcherServiceConfig};

/// Container for all background services
#[derive(Clone)]
pub struct Services {
	/// Filesystem watcher - detects changes and emits events
	pub fs_watcher: Arc<FsWatcherService>,
	/// File sharing service
	pub file_sharing: Arc<FileSharingService>,
	/// Device management service
	pub device: Arc<DeviceService>,
	/// Networking service for device connections
	pub networking: Option<Arc<NetworkingService>>,
	/// Volume monitoring service
	pub volume_monitor: Option<Arc<VolumeMonitorService>>,
	/// Statistics listener service - recalculates library statistics
	pub statistics_listener: Option<Arc<StatisticsListenerService>>,
	/// Sidecar manager
	pub sidecar_manager: Arc<SidecarManager>,
	/// Key manager
	pub key_manager: Arc<KeyManager>,
	/// Shared context for all services
	context: Arc<CoreContext>,
}

impl Services {
	/// Create new services container with context
	pub fn new(context: Arc<CoreContext>) -> Self {
		info!("Initializing background services");

		let fs_watcher_config = FsWatcherServiceConfig::default();
		let fs_watcher = Arc::new(FsWatcherService::new(context.clone(), fs_watcher_config));

		let file_sharing = Arc::new(FileSharingService::new(context.clone()));
		let device = Arc::new(DeviceService::new(context.clone()));
		let sidecar_manager = Arc::new(SidecarManager::new(context.clone()));
		let key_manager = context.key_manager.clone();
		let statistics_listener = Some(Arc::new(StatisticsListenerService::new(context.clone())));
		Self {
			fs_watcher,
			file_sharing,
			device,
			networking: None,     // Initialized separately when needed
			volume_monitor: None, // Initialized after library manager is available
			statistics_listener,
			sidecar_manager,
			key_manager,
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

		// Initialize handlers before starting (connects them to the watcher)
		self.fs_watcher.init_handlers().await;
		self.fs_watcher.start().await?;

		// Start volume monitor if initialized
		if let Some(monitor) = &self.volume_monitor {
			monitor.start().await?;
		}

		Ok(())
	}

	/// Start services based on configuration
	pub async fn start_all_with_config(&self, config: &crate::config::ServiceConfig) -> Result<()> {
		info!("Starting background services based on configuration");

		// Initialize handlers (connects them to the watcher)
		self.fs_watcher.init_handlers().await;

		if config.fs_watcher_enabled {
			self.fs_watcher.start().await?;
		} else {
			info!("Filesystem watcher disabled in configuration");
		}

		// Start volume monitor if initialized and enabled
		if config.volume_monitoring_enabled {
			if let Some(monitor) = &self.volume_monitor {
				monitor.start().await?;
			}
		} else {
			info!("Volume monitoring disabled in configuration");
		}

		// Start networking if initialized and enabled
		if config.networking_enabled {
			if let Some(_networking) = &self.networking {
				self.start_networking().await?;
				info!("Networking service started");
			} else {
				info!("Networking enabled in config but not initialized - call init_networking() first");
			}
		} else {
			info!("Networking disabled in configuration");
		}

		// Start statistics listener if initialized and enabled
		if config.statistics_listener_enabled {
			if let Some(stats) = &self.statistics_listener {
				stats.start().await?;
			}
		} else {
			info!("Statistics listener disabled in configuration");
		}

		Ok(())
	}

	/// Stop all services gracefully
	pub async fn stop_all(&self) -> Result<()> {
		info!("Stopping all background services");

		self.fs_watcher.stop().await?;

		// Stop volume monitor if initialized
		if let Some(monitor) = &self.volume_monitor {
			monitor.stop().await?;
		}

		// Stop statistics listener if initialized
		if let Some(stats) = &self.statistics_listener {
			stats.stop().await?;
		}

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
		key_manager: std::sync::Arc<crate::crypto::key_manager::KeyManager>,
		data_dir: impl AsRef<std::path::Path>,
	) -> Result<()> {
		use crate::service::network::{utils::logging::ConsoleLogger, NetworkingService};

		info!("Initializing networking service");
		let logger = std::sync::Arc::new(ConsoleLogger);
		let networking_service =
			NetworkingService::new(device_manager, key_manager, data_dir, logger)
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
				Arc::as_ptr(networking) as *mut crate::service::network::NetworkingService;
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

	/// Initialize volume monitor service
	pub fn init_volume_monitor(
		&mut self,
		volume_manager: Arc<crate::volume::VolumeManager>,
		library_manager: std::sync::Weak<crate::library::LibraryManager>,
	) {
		info!("Initializing volume monitor service");

		let config = VolumeMonitorConfig::default();
		let volume_monitor = Arc::new(VolumeMonitorService::new(
			volume_manager,
			library_manager,
			config,
		));

		self.volume_monitor = Some(volume_monitor);
	}

	/// Start volume monitor service
	pub async fn start_volume_monitor(&self) -> Result<()> {
		if let Some(monitor) = &self.volume_monitor {
			monitor.start().await?;
		}
		Ok(())
	}

	/// Stop volume monitor service
	pub async fn stop_volume_monitor(&self) -> Result<()> {
		if let Some(monitor) = &self.volume_monitor {
			monitor.stop().await?;
		}
		Ok(())
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
