//! Spacedrive Core v2
//!
//! A unified, simplified architecture for cross-platform file management.

pub mod config;
pub mod device;
pub mod domain;
pub mod file_type;
pub mod infrastructure;
pub mod library;
pub mod location;
pub mod operations;
pub mod services;
pub mod shared;
pub mod volume;

// Re-export networking from infrastructure for backward compatibility
pub use infrastructure::networking;

use crate::config::AppConfig;
use crate::device::DeviceManager;
use crate::infrastructure::events::{Event, EventBus};
use crate::library::LibraryManager;
use crate::services::Services;
use crate::volume::{VolumeDetectionConfig, VolumeManager};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};

/// Pending pairing request information
#[derive(Debug, Clone)]
pub struct PendingPairingRequest {
	pub request_id: uuid::Uuid,
	pub device_id: uuid::Uuid,
	pub device_name: String,
	pub received_at: chrono::DateTime<chrono::Utc>,
}

// NOTE: SimplePairingUI has been moved to CLI infrastructure
// See: src/infrastructure/cli/pairing_ui.rs for CLI-specific implementations

/// The main context for all core operations
pub struct Core {
	/// Application configuration
	config: Arc<RwLock<AppConfig>>,

	/// Device manager
	pub device: Arc<DeviceManager>,

	/// Library manager
	pub libraries: Arc<LibraryManager>,

	/// Volume manager
	pub volumes: Arc<VolumeManager>,

	/// Event bus for state changes
	pub events: Arc<EventBus>,

	/// Background services
	services: Services,

	/// Persistent networking service for device connections
	pub networking: Option<Arc<RwLock<networking::NetworkingService>>>,
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

		// 7. Auto-load all libraries
		info!("Loading existing libraries...");
		match libraries.load_all().await {
			Ok(count) => info!("Loaded {} libraries", count),
			Err(e) => error!("Failed to load libraries: {}", e),
		}

		// 8. Initialize and start services
		let services = Services::new(events.clone());

		info!("Starting background services...");
		match services.start_all().await {
			Ok(()) => info!("Background services started"),
			Err(e) => error!("Failed to start services: {}", e),
		}

		// 9. Emit startup event
		events.emit(Event::CoreStarted);

		Ok(Self {
			config,
			device,
			libraries,
			volumes,
			events,
			services,
			networking: None, // Network will be initialized separately if needed
		})
	}

	/// Get the application configuration
	pub fn config(&self) -> Arc<RwLock<AppConfig>> {
		self.config.clone()
	}

	/// Initialize persistent networking with password
	/// Uses silent logging by default - CLI implementations should use init_networking_with_logger
	pub async fn init_networking(
		&mut self,
		password: &str,
	) -> Result<(), Box<dyn std::error::Error>> {
		// Use silent logger by default in core - CLI implementations should provide their own
		self.init_networking_with_logger(password, Arc::new(networking::SilentLogger)).await
	}

	/// Initialize persistent networking with password and custom logger
	pub async fn init_networking_with_logger(
		&mut self,
		password: &str,
		logger: Arc<dyn networking::NetworkLogger>,
	) -> Result<(), Box<dyn std::error::Error>> {
		logger.info("Initializing persistent networking...").await;

		// Initialize the persistent networking service  
		let mut networking_service =
			networking::init_persistent_networking(self.device.clone(), password).await?;

		// Initialize pairing bridge
		networking_service.init_pairing(password.to_string()).await?;

		// Store the service in the Core
		self.networking = Some(Arc::new(RwLock::new(networking_service)));

		logger.info("Persistent networking initialized successfully").await;
		Ok(())
	}

	/// Initialize persistent networking from Arc<Core> - for daemon use
	pub async fn init_networking_shared(
		core: Arc<Core>,
		password: &str,
	) -> Result<Arc<Core>, Box<dyn std::error::Error>> {
		info!("Initializing persistent networking for shared core...");

		// This is a workaround - in production we'd restructure this differently
		// For now, we'll create a new Core with networking enabled
		let mut new_core = Core::new_with_config(
			core.config().read().await.data_dir.clone()
		).await?;

		// Initialize networking on the new core
		new_core.init_networking(password).await?;

		info!("Persistent networking initialized successfully for shared core");
		Ok(Arc::new(new_core))
	}

	/// Start the networking service (must be called after init_networking)
	pub async fn start_networking(&self) -> Result<(), Box<dyn std::error::Error>> {
		if let Some(networking) = &self.networking {
			info!("Starting persistent networking service...");

			// Start networking service (non-blocking)
			let mut service = networking.write().await;
			if let Err(e) = service.start().await {
				error!("Networking service failed: {}", e);
				return Err(e.into());
			}

			// Note: Event processing will be started when the service needs to handle events
			// For now, we just ensure the service is initialized and ready

			info!("Persistent networking service started");
			Ok(())
		} else {
			Err("Networking not initialized. Call init_networking() first.".into())
		}
	}

	/// Get the networking service (if initialized)
	pub fn networking(&self) -> Option<Arc<RwLock<networking::NetworkingService>>> {
		self.networking.clone()
	}

	/// Get list of connected devices
	pub async fn get_connected_devices(
		&self,
	) -> Result<Vec<uuid::Uuid>, Box<dyn std::error::Error>> {
		if let Some(networking) = &self.networking {
			let service = networking.read().await;
			Ok(service.get_connected_devices().await?)
		} else {
			Ok(Vec::new())
		}
	}

	/// Add a paired device to the network
	pub async fn add_paired_device(
		&self,
		device_info: networking::DeviceInfo,
		session_keys: networking::persistent::SessionKeys,
	) -> Result<(), Box<dyn std::error::Error>> {
		if let Some(networking) = &self.networking {
			let service = networking.read().await;
			service.add_paired_device(device_info, session_keys).await?;
			Ok(())
		} else {
			Err("Networking not initialized".into())
		}
	}

	/// Revoke a paired device
	pub async fn revoke_device(
		&self,
		device_id: uuid::Uuid,
	) -> Result<(), Box<dyn std::error::Error>> {
		if let Some(networking) = &self.networking {
			let service = networking.read().await;
			service.revoke_device(device_id).await?;
			Ok(())
		} else {
			Err("Networking not initialized".into())
		}
	}

	/// Send a file via Spacedrop to a device
	pub async fn send_spacedrop(
		&self,
		device_id: uuid::Uuid,
		file_path: &str,
		sender_name: String,
		message: Option<String>,
	) -> Result<uuid::Uuid, Box<dyn std::error::Error>> {
		if let Some(networking) = &self.networking {
			let service = networking.read().await;

			// Create file metadata
			let metadata = std::fs::metadata(file_path)?;
			let file_metadata = networking::persistent::messages::FileMetadata {
				name: std::path::Path::new(file_path)
					.file_name()
					.unwrap_or_default()
					.to_string_lossy()
					.to_string(),
				size: metadata.len(),
				mime_type: None, // Could be detected
				modified_at: metadata.modified().ok().map(|t| chrono::DateTime::from(t)),
				created_at: metadata.created().ok().map(|t| chrono::DateTime::from(t)),
				is_directory: metadata.is_dir(),
				permissions: None,
				checksum: None, // Could be computed
				extended_attributes: std::collections::HashMap::new(),
			};

			let transfer_id = service
				.send_spacedrop_request(device_id, file_metadata, sender_name, message)
				.await?;

			Ok(transfer_id)
		} else {
			Err("Networking not initialized".into())
		}
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

		self.services
			.location_watcher
			.add_location(watched_location)
			.await?;
		Ok(())
	}

	/// Remove a location from the file system watcher
	pub async fn remove_watched_location(
		&self,
		location_id: uuid::Uuid,
	) -> Result<(), Box<dyn std::error::Error>> {
		self.services
			.location_watcher
			.remove_location(location_id)
			.await?;
		Ok(())
	}

	/// Update file watching settings for a location
	pub async fn update_watched_location(
		&self,
		location_id: uuid::Uuid,
		enabled: bool,
	) -> Result<(), Box<dyn std::error::Error>> {
		self.services
			.location_watcher
			.update_location(location_id, enabled)
			.await?;
		Ok(())
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

		// Stop networking service
		if let Some(_networking) = &self.networking {
			info!("Shutting down networking service...");
			// The networking service will be dropped when Core is dropped
			// Individual connections will be closed gracefully by their drop handlers
		}

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

	/// Start pairing as an initiator (generates pairing code)
	pub async fn start_pairing_as_initiator(
		&self,
		auto_accept: bool,
	) -> Result<(String, u32), Box<dyn std::error::Error>> {
		let networking = self.networking.as_ref()
			.ok_or("Networking not initialized. Call init_networking() first.")?;

		let service = networking.read().await;
		let session = service.start_pairing_as_initiator(auto_accept).await?;
		
		let code = session.code.clone();
		let expires_in = session.expires_in_seconds();
		Ok((code, expires_in))
	}

	/// Start pairing as a joiner (connects using pairing code)
	pub async fn start_pairing_as_joiner(
		&self,
		code: &str,
	) -> Result<(), Box<dyn std::error::Error>> {
		let networking = self.networking.as_ref()
			.ok_or("Networking not initialized. Call init_networking() first.")?;

		let service = networking.read().await;
		service.join_pairing_session(code.to_string()).await?;
		
		Ok(())
	}

	/// Get current pairing status
	pub async fn get_pairing_status(
		&self,
	) -> Result<Vec<networking::persistent::PairingSession>, Box<dyn std::error::Error>> {
		let networking = self.networking.as_ref()
			.ok_or("Networking not initialized. Call init_networking() first.")?;

		let service = networking.read().await;
		Ok(service.get_pairing_status().await)
	}

	/// List pending pairing requests (converted from active pairing sessions)
	pub async fn list_pending_pairings(
		&self,
	) -> Result<Vec<PendingPairingRequest>, Box<dyn std::error::Error>> {
		let sessions = self.get_pairing_status().await?;
		
		// Convert active pairing sessions to pending requests
		let pending_requests: Vec<PendingPairingRequest> = sessions
			.into_iter()
			.filter(|session| matches!(session.status, networking::persistent::PairingStatus::WaitingForConnection))
			.map(|session| PendingPairingRequest {
				request_id: session.id,
				device_id: session.id, // Use session ID as device ID for now
				device_name: "Unknown Device".to_string(), // Would be filled from actual device info
				received_at: chrono::Utc::now(), // Would be actual timestamp
			})
			.collect();
		
		Ok(pending_requests)
	}

	/// Accept a pairing request (cancel pairing session if rejecting)
	pub async fn accept_pairing_request(
		&self,
		request_id: uuid::Uuid,
	) -> Result<(), Box<dyn std::error::Error>> {
		// In the persistent pairing system, acceptance is handled automatically
		// This method exists for API compatibility but doesn't need to do anything
		// since the pairing bridge handles acceptance based on auto_accept flag
		info!("Accepting pairing request: {} (handled automatically by pairing bridge)", request_id);
		Ok(())
	}

	/// Reject a pairing request (cancel the pairing session)
	pub async fn reject_pairing_request(
		&self,
		request_id: uuid::Uuid,
	) -> Result<(), Box<dyn std::error::Error>> {
		let networking = self.networking.as_ref()
			.ok_or("Networking not initialized. Call init_networking() first.")?;

		let service = networking.read().await;
		service.cancel_pairing(request_id).await?;
		
		info!("Rejected pairing request: {}", request_id);
		Ok(())
	}

	/// Get network identity for subprocess helper
	pub async fn get_network_identity(&self) -> Result<networking::NetworkIdentity, Box<dyn std::error::Error>> {
		let networking = self.networking.as_ref()
			.ok_or("Networking not initialized. Call init_networking() first.")?;

		let service = networking.read().await;
		service.get_network_identity().await.map_err(|e| e.into())
	}
}
