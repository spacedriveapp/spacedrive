#![allow(warnings)]
//! Spacedrive Core v2
//!
//! A unified, simplified architecture for cross-platform file management.

pub mod config;
pub mod context;
pub mod device;
pub mod domain;
pub mod file_type;
pub mod infrastructure;
pub mod keys;
pub mod library;
pub mod location;
pub mod operations;
pub mod services;
pub mod shared;
pub mod test_framework;
pub mod volume;

use services::networking::protocols::PairingProtocolHandler;

// Compatibility module for legacy networking references
pub mod networking {
	pub use crate::services::networking::*;
}

use crate::config::AppConfig;
use crate::context::CoreContext;
use crate::device::DeviceManager;
use crate::infrastructure::actions::manager::ActionManager;
use crate::infrastructure::events::{Event, EventBus};
use crate::library::LibraryManager;
use crate::services::Services;
use crate::volume::{VolumeDetectionConfig, VolumeManager};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{error, info};

/// Pending pairing request information
#[derive(Debug, Clone)]
pub struct PendingPairingRequest {
	pub request_id: uuid::Uuid,
	pub device_id: uuid::Uuid,
	pub device_name: String,
	pub received_at: chrono::DateTime<chrono::Utc>,
}

/// Spacedrop request message
#[derive(serde::Serialize, serde::Deserialize)]
struct SpacedropRequest {
	transfer_id: uuid::Uuid,
	file_path: String,
	sender_name: String,
	message: Option<String>,
	file_size: u64,
}

// NOTE: SimplePairingUI has been moved to CLI infrastructure
// See: src/infrastructure/cli/pairing_ui.rs for CLI-specific implementations

/// Bridge between networking events and core events
pub struct NetworkEventBridge {
	network_events: mpsc::UnboundedReceiver<networking::NetworkEvent>,
	core_events: Arc<EventBus>,
}

impl NetworkEventBridge {
	pub fn new(
		network_events: mpsc::UnboundedReceiver<networking::NetworkEvent>,
		core_events: Arc<EventBus>,
	) -> Self {
		Self {
			network_events,
			core_events,
		}
	}

	pub async fn run(mut self) {
		while let Some(event) = self.network_events.recv().await {
			if let Some(core_event) = self.translate_event(event) {
				self.core_events.emit(core_event);
			}
		}
	}

	fn translate_event(&self, event: networking::NetworkEvent) -> Option<Event> {
		match event {
			networking::NetworkEvent::ConnectionEstablished { device_id, .. } => {
				Some(Event::DeviceConnected {
					device_id,
					device_name: "Connected Device".to_string(),
				})
			}
			networking::NetworkEvent::ConnectionLost { device_id, .. } => {
				Some(Event::DeviceDisconnected { device_id })
			}
			networking::NetworkEvent::PairingCompleted {
				device_id,
				device_info,
			} => Some(Event::DeviceConnected {
				device_id,
				device_name: device_info.device_name,
			}),
			_ => None, // Some events don't map to core events
		}
	}
}

/// The main context for all core operations
pub struct Core {
	/// Application configuration
	pub config: Arc<RwLock<AppConfig>>,

	/// Device manager
	pub device: Arc<DeviceManager>,

	/// Library manager
	pub libraries: Arc<LibraryManager>,

	/// Volume manager
	pub volumes: Arc<VolumeManager>,

	/// Event bus for state changes
	pub events: Arc<EventBus>,

	/// Container for high-level services
	pub services: Services,

	/// Shared context for core components
	pub context: Arc<CoreContext>,
}

impl Core {
	/// Initialize a new Core instance with default data directory
	pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
		let data_dir = crate::config::default_data_dir()?;
		Self::new_with_config(data_dir).await
	}

	/// Initialize a new Core instance with custom data directory
	pub async fn new_with_config(data_dir: PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
		info!("Initializing Spacedrive Core at {:?}", data_dir);

		// 1. Load or create app config
		let config = AppConfig::load_or_create(&data_dir)?;
		config.ensure_directories()?;
		let config = Arc::new(RwLock::new(config));

		// 2. Initialize device manager
		let device = Arc::new(DeviceManager::init_with_path(&data_dir)?);
		// Set the global device ID for legacy compatibility
		shared::types::set_current_device_id(device.device_id()?);

		// 3. Create event bus
		let events = Arc::new(EventBus::default());

		// 4. Initialize volume manager
		let volume_config = VolumeDetectionConfig::default();
		let volumes = Arc::new(VolumeManager::new(volume_config, events.clone()));

		// 5. Initialize volume detection
		info!("Initializing volume detection...");
		match volumes.initialize().await {
			Ok(()) => info!("Volume manager initialized"),
			Err(e) => error!("Failed to initialize volume manager: {}", e),
		}

		// 6. Initialize library manager with libraries directory
		let libraries_dir = config.read().await.libraries_dir();
		let libraries = Arc::new(LibraryManager::new_with_dir(libraries_dir, events.clone()));

		// 7. Initialize library key manager
		let library_key_manager =
			Arc::new(crate::keys::library_key_manager::LibraryKeyManager::new()?);

		// 8. Auto-load all libraries
		info!("Loading existing libraries...");
		match libraries.load_all().await {
			Ok(count) => info!("Loaded {} libraries", count),
			Err(e) => error!("Failed to load libraries: {}", e),
		}

		// 9. Register all job types
		info!("Registering job types...");
		crate::operations::register_all_jobs();
		info!("Job types registered");

		// 10. Create the context that will be shared with services
		let context = Arc::new(CoreContext::new(
			events.clone(),
			device.clone(),
			libraries.clone(),
			volumes.clone(),
			library_key_manager.clone(),
		));

		// 11. Initialize services, passing them the context
		let services = Services::new(context.clone());

		info!("Starting background services...");
		match services.start_all().await {
			Ok(()) => info!("Background services started"),
			Err(e) => error!("Failed to start services: {}", e),
		}

		// 12. Initialize ActionManager and set it in context
		let action_manager = Arc::new(crate::infrastructure::actions::manager::ActionManager::new(
			context.clone(),
		));
		context.set_action_manager(action_manager).await;

		// 13. Emit startup event
		events.emit(Event::CoreStarted);

		Ok(Self {
			config,
			device,
			libraries,
			volumes,
			events,
			services,
			context,
		})
	}

	/// Get the application configuration
	pub fn config(&self) -> Arc<RwLock<AppConfig>> {
		self.config.clone()
	}

	/// Initialize networking using master key
	pub async fn init_networking(&mut self) -> Result<(), Box<dyn std::error::Error>> {
		self.init_networking_with_logger(Arc::new(networking::SilentLogger))
			.await
	}

	/// Initialize networking with custom logger
	pub async fn init_networking_with_logger(
		&mut self,
		logger: Arc<dyn networking::NetworkLogger>,
	) -> Result<(), Box<dyn std::error::Error>> {
		logger.info("Initializing networking...").await;

		// Initialize networking service through the services container
		let data_dir = self.config.read().await.data_dir.clone();
		self.services
			.init_networking(
				self.device.clone(),
				self.services.library_key_manager.clone(),
				data_dir,
			)
			.await?;

		// Start the networking service
		self.services.start_networking().await?;

		// Get the networking service for protocol registration
		if let Some(networking_service) = self.services.networking() {
			// Register default protocol handlers
			self.register_default_protocols(&networking_service).await?;

			// Set up event bridge to integrate with core event system
			let event_bridge = NetworkEventBridge::new(
				networking_service
					.subscribe_events()
					.await
					.unwrap_or_else(|| {
						let (_, rx) = tokio::sync::mpsc::unbounded_channel();
						rx
					}),
				self.events.clone(),
			);
			tokio::spawn(event_bridge.run());
		}

		logger.info("Networking initialized successfully").await;
		Ok(())
	}

	/// Register default protocol handlers
	async fn register_default_protocols(
		&self,
		networking: &networking::NetworkingService,
	) -> Result<(), Box<dyn std::error::Error>> {
		let logger = std::sync::Arc::new(networking::utils::logging::ConsoleLogger);

		// Get command sender for the pairing handler's state machine
		let command_sender = networking
			.command_sender()
			.ok_or("NetworkingEventLoop command sender not available")?
			.clone();

		let pairing_handler = Arc::new(networking::protocols::PairingProtocolHandler::new(
			networking.identity().clone(),
			networking.device_registry(),
			logger,
			command_sender,
		));

		// Start the state machine task for pairing
		networking::protocols::PairingProtocolHandler::start_state_machine_task(
			pairing_handler.clone(),
		);

		// Start cleanup task for expired sessions
		networking::protocols::PairingProtocolHandler::start_cleanup_task(pairing_handler.clone());

		let messaging_handler = networking::protocols::MessagingProtocolHandler::new();
		let mut file_transfer_handler =
			networking::protocols::FileTransferProtocolHandler::new_default();

		// Inject device registry into file transfer handler for encryption
		file_transfer_handler.set_device_registry(networking.device_registry());

		let protocol_registry = networking.protocol_registry();
		{
			let mut registry = protocol_registry.write().await;
			registry.register_handler(pairing_handler)?;
			registry.register_handler(Arc::new(messaging_handler))?;
			registry.register_handler(Arc::new(file_transfer_handler))?;
		}

		Ok(())
	}

	/// Initialize networking from Arc<Core> - for daemon use
	pub async fn init_networking_shared(
		core: Arc<Core>,
	) -> Result<Arc<Core>, Box<dyn std::error::Error>> {
		info!("Initializing networking for shared core...");

		// Create a new Core with networking enabled
		let mut new_core =
			Core::new_with_config(core.config().read().await.data_dir.clone()).await?;

		// Initialize networking on the new core
		new_core.init_networking().await?;

		info!("Networking initialized successfully for shared core");
		Ok(Arc::new(new_core))
	}

	/// Start the networking service (must be called after init_networking)
	pub async fn start_networking(&self) -> Result<(), Box<dyn std::error::Error>> {
		if let Some(_networking) = self.services.networking() {
			// Networking is already started in init_networking
			info!("Networking system is active and ready");
			Ok(())
		} else {
			Err("Networking not initialized. Call init_networking() first.".into())
		}
	}

	/// Get the networking service (if initialized)
	pub fn networking(&self) -> Option<Arc<networking::NetworkingService>> {
		self.services.networking()
	}

	/// Get list of connected devices
	pub async fn get_connected_devices(
		&self,
	) -> Result<Vec<uuid::Uuid>, Box<dyn std::error::Error>> {
		Ok(self.services.device.get_connected_devices().await?)
	}

	/// Get detailed information about connected devices
	pub async fn get_connected_devices_info(
		&self,
	) -> Result<Vec<networking::DeviceInfo>, Box<dyn std::error::Error>> {
		Ok(self.services.device.get_connected_devices_info().await?)
	}

	/// Add a location to the file system watcher
	pub async fn add_watched_location(
		&self,
		location_id: uuid::Uuid,
		library_id: uuid::Uuid,
		path: std::path::PathBuf,
		enabled: bool,
	) -> Result<(), Box<dyn std::error::Error>> {
		use crate::services::location_watcher::WatchedLocation;

		let watched_location = WatchedLocation {
			id: location_id,
			library_id,
			path,
			enabled,
		};

		Ok(self
			.services
			.location_watcher
			.add_location(watched_location)
			.await?)
	}

	/// Remove a location from the file system watcher
	pub async fn remove_watched_location(
		&self,
		location_id: uuid::Uuid,
	) -> Result<(), Box<dyn std::error::Error>> {
		Ok(self
			.services
			.location_watcher
			.remove_location(location_id)
			.await?)
	}

	/// Update file watching settings for a location
	pub async fn update_watched_location(
		&self,
		location_id: uuid::Uuid,
		enabled: bool,
	) -> Result<(), Box<dyn std::error::Error>> {
		Ok(self
			.services
			.location_watcher
			.update_location(location_id, enabled)
			.await?)
	}

	/// Get all currently watched locations
	pub async fn get_watched_locations(
		&self,
	) -> Vec<crate::services::location_watcher::WatchedLocation> {
		self.services.location_watcher.get_watched_locations().await
	}

	/// Shutdown the core gracefully
	pub async fn shutdown(&self) -> Result<(), Box<dyn std::error::Error>> {
		info!("Shutting down Spacedrive Core...");

		// Networking service is stopped by services.stop_all()

		// Stop all services
		self.services.stop_all().await?;

		// Stop volume monitoring
		self.volumes.stop_monitoring().await;

		// Close all libraries
		self.libraries.close_all().await?;

		// Save configuration
		self.config.write().await.save()?;

		// Emit shutdown event
		self.events.emit(Event::CoreShutdown);

		info!("Spacedrive Core shutdown complete");
		Ok(())
	}
}
