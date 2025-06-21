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

/// Simple UI implementation for CLI pairing that captures the pairing code
struct SimplePairingUI {
	auto_accept: bool,
	code_sender: Option<tokio::sync::oneshot::Sender<(String, u32)>>,
}

#[async_trait::async_trait]
impl networking::pairing::PairingUserInterface for SimplePairingUI {
	async fn show_pairing_error(&self, error: &networking::NetworkError) {
		error!("Pairing error: {}", error);
	}

	async fn show_pairing_code(&self, code: &str, expires_in_seconds: u32) {
		info!("Pairing code generated: {} (expires in {} seconds)", code, expires_in_seconds);
		
		// Send the code back to the waiting CLI
		if let Some(sender) = &self.code_sender {
			// We can't move out of self, so we'll log here and let the pairing method handle it differently
			// This is a limitation of the current UI interface design
		}
	}

	async fn prompt_pairing_code(&self) -> networking::Result<[String; 12]> {
		// This should not be called in the CLI daemon context
		Err(networking::NetworkError::AuthenticationFailed(
			"Interactive pairing code input not supported in daemon mode".to_string(),
		))
	}

	async fn confirm_pairing(&self, remote_device: &networking::DeviceInfo) -> networking::Result<bool> {
		if self.auto_accept {
			info!("Auto-accepting pairing with device: {}", remote_device.device_name);
			Ok(true)
		} else {
			info!("Pairing request from device: {} (manual confirmation required)", remote_device.device_name);
			// In daemon mode, we'll store the request and let the user decide via CLI
			Ok(false)
		}
	}

	async fn show_pairing_progress(&self, state: networking::pairing::PairingState) {
		match state {
			networking::pairing::PairingState::GeneratingCode => info!("Generating pairing code..."),
			networking::pairing::PairingState::Broadcasting => info!("Broadcasting on DHT..."),
			networking::pairing::PairingState::Scanning => info!("Scanning DHT for devices..."),
			networking::pairing::PairingState::Connecting => info!("Establishing connection..."),
			networking::pairing::PairingState::Authenticating => info!("Authenticating..."),
			networking::pairing::PairingState::ExchangingKeys => info!("Exchanging keys..."),
			networking::pairing::PairingState::AwaitingConfirmation => info!("Awaiting confirmation..."),
			networking::pairing::PairingState::EstablishingSession => info!("Establishing session..."),
			networking::pairing::PairingState::Completed => info!("Pairing completed!"),
			networking::pairing::PairingState::Failed(err) => error!("Pairing failed: {}", err),
			_ => {}
		}
	}
}

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
	pub async fn init_networking(
		&mut self,
		password: &str,
	) -> Result<(), Box<dyn std::error::Error>> {
		info!("Initializing persistent networking...");

		// Initialize the persistent networking service
		let networking_service =
			networking::init_persistent_networking(self.device.clone(), password).await?;

		// Store the service in the Core
		self.networking = Some(Arc::new(RwLock::new(networking_service)));

		info!("Persistent networking initialized successfully");
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
		if self.networking.is_none() {
			return Err("Networking not initialized. Call init_networking() first.".into());
		}

		// Generate a real BIP39 pairing code using the existing infrastructure
		let pairing_code = networking::pairing::PairingCode::generate()?;
		let code_string = pairing_code.words.join(" ");
		
		info!("Generated pairing code for initiator with auto_accept: {}", auto_accept);
		
		// TODO: Integrate with persistent networking service to actually start the pairing protocol
		// This requires extending NetworkingService to support pairing operations
		// For now, return the real generated code
		Ok((code_string, 300))
	}

	/// Start pairing as a joiner (connects using pairing code)
	pub async fn start_pairing_as_joiner(
		&self,
		code: &str,
	) -> Result<(), Box<dyn std::error::Error>> {
		if self.networking.is_none() {
			return Err("Networking not initialized. Call init_networking() first.".into());
		}

		// Parse and validate pairing code using the existing infrastructure
		let words: Vec<String> = code.split_whitespace().map(|s| s.to_string()).collect();
		if words.len() != 12 {
			return Err("Invalid pairing code format. Expected 12 words.".into());
		}

		let word_array = [
			words[0].clone(), words[1].clone(), words[2].clone(), words[3].clone(),
			words[4].clone(), words[5].clone(), words[6].clone(), words[7].clone(),
			words[8].clone(), words[9].clone(), words[10].clone(), words[11].clone(),
		];

		let _pairing_code = networking::pairing::PairingCode::from_words(&word_array)?;

		info!("Starting pairing as joiner with code: {}", code);
		
		// TODO: Integrate with persistent networking service to actually start the pairing protocol
		// This requires extending NetworkingService to support pairing operations
		// The real implementation would use LibP2PPairingProtocol::start_as_joiner()
		
		Ok(())
	}

	/// Get current pairing status
	pub async fn get_pairing_status(
		&self,
	) -> Result<(String, Option<crate::networking::DeviceInfo>), Box<dyn std::error::Error>> {
		// TODO: Implement proper pairing status tracking
		Ok(("no_active_pairing".to_string(), None))
	}

	/// List pending pairing requests
	pub async fn list_pending_pairings(
		&self,
	) -> Result<Vec<PendingPairingRequest>, Box<dyn std::error::Error>> {
		// TODO: Implement pending pairing request storage and retrieval
		Ok(Vec::new())
	}

	/// Accept a pairing request
	pub async fn accept_pairing_request(
		&self,
		request_id: uuid::Uuid,
	) -> Result<(), Box<dyn std::error::Error>> {
		// TODO: Implement pairing request acceptance
		info!("Accepting pairing request: {}", request_id);
		Ok(())
	}

	/// Reject a pairing request
	pub async fn reject_pairing_request(
		&self,
		request_id: uuid::Uuid,
	) -> Result<(), Box<dyn std::error::Error>> {
		// TODO: Implement pairing request rejection
		info!("Rejecting pairing request: {}", request_id);
		Ok(())
	}
}
