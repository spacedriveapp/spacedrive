#![allow(warnings)]
//! Spacedrive Core v2
//!
//! A Virtual Distributed File System (VDFS) implementation in Rust.

pub mod client;
pub mod common;
pub mod config;
pub mod context;
pub mod crypto;
pub mod device;
pub mod domain;
pub mod filetype;
pub mod infra;
pub mod library;
pub mod location;
pub mod ops;
pub mod service;
pub mod testing;
pub mod volume;

use crate::{
	config::AppConfig,
	context::CoreContext,
	device::DeviceManager,
	infra::{
		action::{builder::ActionBuilder, manager::ActionManager, CoreAction, LibraryAction},
		api::ApiDispatcher,
		event::{Event, EventBus},
		query::QueryManager,
	},
	library::LibraryManager,
	service::session::SessionStateService,
	service::{
		network::{protocol::pairing::PairingProtocolHandler, utils::logging::NetworkLogger},
		Services,
	},
	volume::{VolumeDetectionConfig, VolumeManager},
};

use std::{path::PathBuf, sync::Arc};
use tokio::sync::{mpsc, RwLock};
use tracing::{error, info, warn};
use uuid::Uuid;

/// The main context for all core operations
#[derive(Clone)]
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

	/// WASM plugin manager
	pub plugin_manager: Option<Arc<RwLock<crate::infra::extension::PluginManager>>>,

	/// Shared context for core components
	pub context: Arc<CoreContext>,

	/// Unified API dispatcher for enhanced operations
	api_dispatcher: ApiDispatcher,
}

impl Core {
	/// Initialize a new Core instance with custom data directory
	pub async fn new(data_dir: PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
		Self::new_with_device_name(data_dir, None).await
	}

	/// Initialize a new Core instance with custom data directory and optional device name
	///
	/// This is primarily for mobile platforms (iOS, Android) where the device name
	/// should be provided by the native platform APIs (e.g., UIDevice.name on iOS)
	pub async fn new_with_device_name(
		data_dir: PathBuf,
		device_name: Option<String>,
	) -> Result<Self, Box<dyn std::error::Error>> {
		info!("Initializing Spacedrive at {:?}", data_dir);

		// Load or create app config
		let config = AppConfig::load_or_create(&data_dir)?;

		config.ensure_directories()?;

		let config = Arc::new(RwLock::new(config));

		// Initialize device manager
		let device = Arc::new(DeviceManager::init_with_path_and_name(
			&data_dir,
			device_name,
		)?);

		// Set a global device ID for convenience
		crate::device::set_current_device_id(device.device_id()?);

		// Create event bus
		let events = Arc::new(EventBus::default());

		// Initialize volume manager
		let volume_config = VolumeDetectionConfig::default();
		let device_id = device.device_id()?;
		let volumes = Arc::new(VolumeManager::new(device_id, volume_config, events.clone()));

		// Initialize volume detection (if enabled)
		let config_read = config.read().await;
		if config_read.services.volume_monitoring_enabled {
			info!("Initializing volume detection...");
			match volumes.initialize().await {
				Ok(()) => info!("Volume manager initialized"),
				Err(e) => error!("Failed to initialize volume manager: {}", e),
			}
		} else {
			info!("Volume monitoring disabled in configuration");
		}
		drop(config_read);

		// Initialize library key manager
		let library_key_manager =
			Arc::new(crate::crypto::library_key_manager::LibraryKeyManager::new()?);

		// Create the context that will be shared with services
		let mut context_inner = CoreContext::new(
			events.clone(),
			device.clone(),
			None, // Libraries will be set after context creation
			volumes.clone(),
			library_key_manager.clone(),
		);

		// Enable per-job file logging by default
		let mut app_config = config.write().await;
		if !app_config.job_logging.enabled {
			app_config.job_logging.enabled = true;
		}
		// Ensure directory exists and apply to context
		let logs_dir = app_config.job_logs_dir();
		let _ = std::fs::create_dir_all(&logs_dir);
		context_inner.set_job_logging(app_config.job_logging.clone(), logs_dir);
		drop(app_config);

		// Create the shared context
		let context = Arc::new(context_inner);

		// Initialize library manager with libraries directory and context
		let libraries_dir = config.read().await.libraries_dir();
		let libraries = Arc::new(LibraryManager::new_with_dir(
			libraries_dir,
			events.clone(),
			volumes.clone(),
			device.clone(),
		));

		// Update context with libraries
		context.set_libraries(libraries.clone()).await;

		// Initialize services first, passing them the context
		let mut services = Services::new(context.clone());

		// Auto-load all libraries with context for job manager initialization
		info!("Loading existing libraries...");
		let mut loaded_libraries: Vec<Arc<crate::library::Library>> =
			match libraries.load_all(context.clone()).await {
				Ok(count) => {
					info!("Loaded {} libraries", count);
					libraries.list().await
				}
				Err(e) => {
					error!("Failed to load libraries: {}", e);
					vec![]
				}
			};

		// Create default library if no libraries exist
		if loaded_libraries.is_empty() {
			info!("No existing libraries found, creating default library 'My Library'");
			match libraries
				.create_library("My Library", None, context.clone())
				.await
			{
				Ok(default_library) => {
					info!("Created default library: {}", default_library.id());
					loaded_libraries.push(default_library);
				}
				Err(e) => {
					error!("Failed to create default library: {}", e);
				}
			}
		}

		// Initialize sidecar manager for each loaded library
		for library in &loaded_libraries {
			info!("Initializing sidecar manager for library {}", library.id());
			if let Err(e) = services.sidecar_manager.init_library(&library).await {
				error!(
					"Failed to initialize sidecar manager for library {}: {}",
					library.id(),
					e
				);
			} else {
				// Run bootstrap scan
				if let Err(e) = services.sidecar_manager.bootstrap_scan(&library).await {
					error!(
						"Failed to run sidecar bootstrap scan for library {}: {}",
						library.id(),
						e
					);
				}
			}
		}

		// Initialize networking if enabled in config
		let service_config = config.read().await.services.clone();
		if service_config.networking_enabled {
			info!("Initializing networking service...");
			match services
				.init_networking(
					device.clone(),
					services.library_key_manager.clone(),
					config.read().await.data_dir.clone(),
				)
				.await
			{
				Ok(()) => {
					info!("Networking service initialized");
					// Store networking service in context so it can be accessed
					if let Some(networking) = services.networking() {
						context.set_networking(networking.clone()).await;
						info!("Networking service registered in context");

						// Initialize sync service on already-loaded libraries
						// (libraries were loaded before networking was available)
						info!(
							"Initializing sync service on {} loaded libraries...",
							loaded_libraries.len()
						);
						for library in &loaded_libraries {
							if library.sync_service().is_some() {
								info!(
									"Sync service already initialized for library {}",
									library.id()
								);
								continue;
							}

							match library
								.init_sync_service(device_id, networking.clone())
								.await
							{
								Ok(()) => {
									info!("Sync service initialized for library {}", library.id());
								}
								Err(e) => {
									warn!(
										"Failed to initialize sync service for library {}: {}",
										library.id(),
										e
									);
								}
							}
						}
					}
				}
				Err(e) => {
					error!("Failed to initialize networking: {}", e);
					// Continue without networking
				}
			}
		}

		info!("Starting background services...");
		match services.start_all_with_config(&service_config).await {
			Ok(()) => info!("Background services started"),
			Err(e) => error!("Failed to start services: {}", e),
		}

		//Initialize ActionManager and set it in context
		let action_manager = Arc::new(crate::infra::action::manager::ActionManager::new(
			context.clone(),
		));
		context.set_action_manager(action_manager).await;

		// Set up log event emitter
		setup_log_event_emitter(events.clone());

		// Initialize API dispatcher
		let api_dispatcher = ApiDispatcher::new(context.clone());

		// Initialize plugin manager (WASM extensions)
		let plugin_dir = data_dir.join("extensions");
		let _ = std::fs::create_dir_all(&plugin_dir); // Ensure directory exists

		let plugin_manager = Arc::new(RwLock::new(crate::infra::extension::PluginManager::new(
			plugin_dir,
			context.clone(),
			Arc::new(api_dispatcher.clone()),
		)));

		// Set in context so jobs can access it
		context.set_plugin_manager(plugin_manager.clone()).await;

		events.emit(Event::CoreStarted);

		Ok(Self {
			config,
			device,
			libraries,
			volumes,
			events,
			services,
			plugin_manager: Some(plugin_manager),
			context,
			api_dispatcher,
		})
	}

	/// Get the application configuration
	pub fn config(&self) -> Arc<RwLock<AppConfig>> {
		self.config.clone()
	}

	/// Initialize networking using master key
	pub async fn init_networking(&mut self) -> Result<(), Box<dyn std::error::Error>> {
		self.init_networking_with_logger(Arc::new(service::network::SilentLogger))
			.await
	}

	/// Initialize networking with custom logger
	pub async fn init_networking_with_logger(
		&mut self,
		logger: Arc<dyn service::network::NetworkLogger>,
	) -> Result<(), Box<dyn std::error::Error>> {
		logger.info("Initializing networking...").await;

		// Check if networking is already initialized
		let already_initialized = self.services.networking().is_some();

		if !already_initialized {
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
		} else {
			logger
				.info("Networking already initialized, skipping service creation")
				.await;
		}

		// Register protocols and set up event bridge
		if let Some(networking_service) = self.services.networking() {
			// Check if protocols are already registered by checking handler count
			let handler_count = {
				let registry = networking_service.protocol_registry();
				let registry_guard = registry.read().await;
				registry_guard.handler_count()
			};

			// Register default protocol handlers if not already registered
			if handler_count == 0 {
				logger.info("Registering protocol handlers...").await;
				self.register_default_protocols(&networking_service).await?;
			} else {
				logger
					.info(&format!(
						"Protocol handlers already registered ({} handlers)",
						handler_count
					))
					.await;
			}

			// Set up event bridge to integrate with core event system (only if not already done)
			if !already_initialized {
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

			// Make networking service available to the context for other services
			self.context.set_networking(networking_service).await;
		}

		logger.info("Networking initialized successfully").await;
		Ok(())
	}

	/// Register default protocol handlers
	async fn register_default_protocols(
		&self,
		networking: &service::network::NetworkingService,
	) -> Result<(), Box<dyn std::error::Error>> {
		let logger = std::sync::Arc::new(service::network::utils::logging::ConsoleLogger);

		// Get command sender for the pairing handler's state machine
		let command_sender = networking
			.command_sender()
			.ok_or("NetworkingEventLoop command sender not available")?
			.clone();

		// Get data directory from config
		let data_dir = {
			let config = self.config.read().await;
			config.data_dir.clone()
		};

		let pairing_handler = Arc::new(
			service::network::protocol::PairingProtocolHandler::new_with_persistence(
				networking.identity().clone(),
				networking.device_registry(),
				logger.clone(),
				command_sender,
				data_dir,
			),
		);

		// Try to load persisted sessions, but don't fail if there's an error
		if let Err(e) = pairing_handler.load_persisted_sessions().await {
			logger
				.warn(&format!(
					"Failed to load persisted pairing sessions: {}. Starting with empty sessions.",
					e
				))
				.await;
		}

		// Start the state machine task for pairing
		service::network::protocol::PairingProtocolHandler::start_state_machine_task(
			pairing_handler.clone(),
		);

		// Start cleanup task for expired sessions
		service::network::protocol::PairingProtocolHandler::start_cleanup_task(
			pairing_handler.clone(),
		);

		let mut messaging_handler = service::network::protocol::MessagingProtocolHandler::new();

		// Inject context for library operations
		messaging_handler.set_context(self.context.clone());

		let mut file_transfer_handler =
			service::network::protocol::FileTransferProtocolHandler::new_default(logger.clone());

		// Inject device registry into file transfer handler for encryption
		file_transfer_handler.set_device_registry(networking.device_registry());

		let protocol_registry = networking.protocol_registry();
		{
			let mut registry = protocol_registry.write().await;
			registry.register_handler(pairing_handler)?;
			registry.register_handler(Arc::new(messaging_handler))?;
			registry.register_handler(Arc::new(file_transfer_handler))?;
			logger
				.info("All protocol handlers registered successfully")
				.await;
		}

		// Brief delay to ensure protocol handlers are fully initialized and background
		// tasks have started before accepting connections. This prevents race conditions
		// where incoming connections arrive before handlers are ready.
		// 50ms is imperceptible to users but sufficient for async task scheduling.
		tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

		Ok(())
	}

	/// Get the networking service (if initialized)
	pub fn networking(&self) -> Option<Arc<service::network::NetworkingService>> {
		self.services.networking()
	}

	/// Get the unified API dispatcher
	///
	/// This is the main entry point for enhanced operations with session context,
	/// permissions, and audit trails. Prefer this over direct registry access.
	pub fn api(&self) -> &ApiDispatcher {
		&self.api_dispatcher
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

/// Set up log event emitter to forward tracing events to the event bus
fn setup_log_event_emitter(event_bus: Arc<crate::infra::event::EventBus>) {
	use crate::infra::event::log_emitter::LogEventLayer;
	use std::sync::Once;
	use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

	static SETUP: Once = Once::new();

	SETUP.call_once(|| {
		// Create the log event layer (now global bus is set elsewhere)
		let log_layer = LogEventLayer::new();

		// Try to add it to the existing global subscriber
		// Since we can't modify an existing subscriber, we'll set up a new one
		// This will only work if no subscriber has been set yet
		let _ = tracing_subscriber::registry().with(log_layer).try_init();
	});
}

// Compatibility module for legacy networking references
pub mod networking {
	pub use crate::service::network::*;
}

/// Bridge between networking events and core events
/// TODO: why? - james
pub struct NetworkEventBridge {
	network_events: mpsc::UnboundedReceiver<service::network::NetworkEvent>,
	core_events: Arc<EventBus>,
}

impl NetworkEventBridge {
	pub fn new(
		network_events: mpsc::UnboundedReceiver<service::network::NetworkEvent>,
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

	fn translate_event(&self, event: service::network::NetworkEvent) -> Option<Event> {
		match event {
			service::network::NetworkEvent::ConnectionEstablished { device_id, .. } => {
				Some(Event::DeviceConnected {
					device_id,
					device_name: "Connected Device".to_string(),
				})
			}
			service::network::NetworkEvent::ConnectionLost { device_id, .. } => {
				Some(Event::DeviceDisconnected { device_id })
			}
			service::network::NetworkEvent::PairingCompleted {
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
