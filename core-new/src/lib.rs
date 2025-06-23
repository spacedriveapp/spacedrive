#![allow(warnings)]
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

pub use infrastructure::networking;
use infrastructure::networking::protocols::PairingProtocolHandler;

use crate::config::AppConfig;
use crate::device::DeviceManager;
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

	/// Networking service for device connections
	pub networking: Option<Arc<RwLock<networking::NetworkingCore>>>,
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

	/// Initialize networking with password
	pub async fn init_networking(
		&mut self,
		_password: &str,
	) -> Result<(), Box<dyn std::error::Error>> {
		self.init_networking_with_logger(Arc::new(networking::SilentLogger))
			.await
	}

	/// Initialize networking with custom logger
	pub async fn init_networking_with_logger(
		&mut self,
		logger: Arc<dyn networking::NetworkLogger>,
	) -> Result<(), Box<dyn std::error::Error>> {
		logger.info("Initializing networking...").await;

		// Initialize the new networking core
		let mut networking_core = networking::NetworkingCore::new(self.device.clone()).await?;

		// Register default protocol handlers
		self.register_default_protocols(&networking_core).await?;

		// Start networking
		networking_core.start().await?;

		// Set up event bridge to integrate with core event system
		let event_bridge = NetworkEventBridge::new(
			networking_core.subscribe_events().await.unwrap_or_else(|| {
				let (_, rx) = tokio::sync::mpsc::unbounded_channel();
				rx
			}),
			self.events.clone(),
		);
		tokio::spawn(event_bridge.run());

		// Store the networking core
		self.networking = Some(Arc::new(RwLock::new(networking_core)));

		logger.info("Networking initialized successfully").await;
		Ok(())
	}

	/// Register default protocol handlers
	async fn register_default_protocols(
		&self,
		networking: &networking::NetworkingCore,
	) -> Result<(), Box<dyn std::error::Error>> {
		let pairing_handler = networking::protocols::PairingProtocolHandler::new(
			networking.identity().clone(),
			networking.device_registry(),
		);

		let messaging_handler = networking::protocols::MessagingProtocolHandler::new();

		let protocol_registry = networking.protocol_registry();
		{
			let mut registry = protocol_registry.write().await;
			registry.register_handler(Arc::new(pairing_handler))?;
			registry.register_handler(Arc::new(messaging_handler))?;
		}

		Ok(())
	}

	/// Initialize networking from Arc<Core> - for daemon use
	pub async fn init_networking_shared(
		core: Arc<Core>,
		password: &str,
	) -> Result<Arc<Core>, Box<dyn std::error::Error>> {
		info!("Initializing networking for shared core...");

		// Create a new Core with networking enabled
		let mut new_core =
			Core::new_with_config(core.config().read().await.data_dir.clone()).await?;

		// Initialize networking on the new core
		new_core.init_networking(password).await?;

		info!("Networking initialized successfully for shared core");
		Ok(Arc::new(new_core))
	}

	/// Start the networking service (must be called after init_networking)
	pub async fn start_networking(&self) -> Result<(), Box<dyn std::error::Error>> {
		if let Some(_networking) = &self.networking {
			// Networking is already started in init_networking
			info!("Networking system is active and ready");
			Ok(())
		} else {
			Err("Networking not initialized. Call init_networking() first.".into())
		}
	}

	/// Get the networking service (if initialized)
	pub fn networking(&self) -> Option<Arc<RwLock<networking::NetworkingCore>>> {
		self.networking.clone()
	}

	/// Get list of connected devices
	pub async fn get_connected_devices(
		&self,
	) -> Result<Vec<uuid::Uuid>, Box<dyn std::error::Error>> {
		if let Some(networking) = &self.networking {
			let service = networking.read().await;
			let devices = service.get_connected_devices().await;
			Ok(devices.into_iter().map(|d| d.device_id).collect())
		} else {
			Ok(Vec::new())
		}
	}

	/// Add a paired device to the network
	pub async fn add_paired_device(
		&self,
		device_info: networking::DeviceInfo,
		session_keys: networking::device::SessionKeys,
	) -> Result<(), Box<dyn std::error::Error>> {
		if let Some(networking) = &self.networking {
			let service = networking.read().await;
			let device_registry = service.device_registry();
			{
				let mut registry = device_registry.write().await;
				registry.complete_pairing(device_info.device_id, device_info, session_keys)?;
			}
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
			let device_registry = service.device_registry();
			{
				let mut registry = device_registry.write().await;
				registry.remove_device(device_id)?;
			}
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

			// Create spacedrop request message
			let transfer_id = uuid::Uuid::new_v4();
			let spacedrop_request = SpacedropRequest {
				transfer_id,
				file_path: file_path.to_string(),
				sender_name,
				message,
				file_size: std::fs::metadata(file_path)?.len(),
			};

			// Send via messaging protocol
			service
				.send_message(
					device_id,
					"spacedrop",
					serde_json::to_vec(&spacedrop_request)?,
				)
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
		let networking = self
			.networking
			.as_ref()
			.ok_or("Networking not initialized. Call init_networking() first.")?;

		// Get pairing handler from protocol registry
		let service = networking.read().await;
		let registry = service.protocol_registry();
		let pairing_handler = registry
			.read()
			.await
			.get_handler("pairing")
			.ok_or("Pairing protocol not registered")?;

		// Cast to pairing handler to access pairing-specific methods
		let pairing_handler = pairing_handler
			.as_any()
			.downcast_ref::<networking::protocols::PairingProtocolHandler>()
			.ok_or("Invalid pairing handler type")?;

		// Start pairing session first to get the actual session ID
		let actual_session_id = pairing_handler.start_pairing_session().await?;

		// Generate BIP39 pairing code using the actual session ID  
		let pairing_code = networking::protocols::pairing::PairingCode::from_session_id(actual_session_id);

		// Create pairing advertisement for DHT
		let advertisement = networking::protocols::pairing::PairingAdvertisement {
			peer_id: service.peer_id().to_string(),
			addresses: service.get_external_addresses().await.into_iter()
				.map(|addr| addr.to_string())
				.collect(),
			device_info: pairing_handler.get_device_info(),
			expires_at: chrono::Utc::now() + chrono::Duration::minutes(5),
			created_at: chrono::Utc::now(),
		};

		// CRITICAL FIX: Use actual session ID for DHT key (not pairing code session ID)
		let key = libp2p::kad::RecordKey::new(&actual_session_id.as_bytes());
		let value = serde_json::to_vec(&advertisement)?;
		
		let query_id = service.publish_dht_record(key, value).await?;
		println!("Published pairing session to DHT: session={}, query_id={:?}", actual_session_id, query_id);

		let expires_in = 300; // 5 minutes

		Ok((pairing_code.to_string(), expires_in))
	}

	/// Start pairing as a joiner (connects using pairing code)
	pub async fn start_pairing_as_joiner(
		&self,
		code: &str,
	) -> Result<(), Box<dyn std::error::Error>> {
		let networking = self
			.networking
			.as_ref()
			.ok_or("Networking not initialized. Call init_networking() first.")?;

		// Parse BIP39 pairing code
		let pairing_code = networking::protocols::pairing::PairingCode::from_string(code)?;
		let session_id = pairing_code.session_id();

		let service = networking.read().await;

		// CRITICAL FIX: Join Alice's pairing session using her session ID
		let registry = service.protocol_registry();
		let pairing_handler = registry.read().await.get_handler("pairing")
			.ok_or("Pairing protocol not registered")?;
		let pairing_handler = pairing_handler
			.as_any()
			.downcast_ref::<networking::protocols::PairingProtocolHandler>()
			.ok_or("Invalid pairing handler type")?;
		
		// Join Alice's pairing session using the session ID from the pairing code
		pairing_handler.join_pairing_session(session_id).await?;
		println!("Bob joined Alice's pairing session: {}", session_id);

		// Verify Bob's session was created correctly
		let bob_sessions = pairing_handler.get_active_sessions().await;
		let bob_session = bob_sessions.iter().find(|s| s.id == session_id);
		match bob_session {
			Some(session) => {
				println!("✅ Bob's session verified: {} in state {:?}", session.id, session.state);
				if !matches!(session.state, networking::protocols::pairing::PairingState::Scanning) {
					return Err(format!("Bob's session is in wrong state: {:?}, expected Scanning", session.state).into());
				}
			}
			None => {
				return Err("Failed to create Bob's pairing session".into());
			}
		}

		// PRODUCTION FIX: Wait for mDNS discovery to trigger direct pairing attempts
		// The mDNS event loop needs time to see Bob's new Scanning session and send pairing requests
		println!("⏳ Waiting for mDNS discovery to trigger pairing requests...");
		tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

		// Hybrid approach: Try local pairing first, then DHT fallback for remote pairing
		
		// 1. Attempt direct pairing with any currently connected peers (local pairing)
		// This handles the common case where Alice and Bob are on the same network
		let connected_peers = service.get_connected_peers().await;
		if !connected_peers.is_empty() {
			println!("Attempting direct pairing with {} connected peers for session: {}", 
					 connected_peers.len(), session_id);
			
			// Send pairing requests to all connected peers
			// One of them might be Alice with the matching session
			for peer_id in connected_peers {
				let pairing_request = networking::core::behavior::PairingMessage::PairingRequest {
					session_id,
					device_id: service.device_id(),
					device_name: "Bob's Device".to_string(), // TODO: Get from device manager
					public_key: service.identity().public_key_bytes(),
				};
				
				match service.send_message_to_peer(
					peer_id, 
					"pairing", 
					serde_json::to_vec(&pairing_request).unwrap_or_default()
				).await {
					Ok(_) => println!("Sent direct pairing request to peer: {}", peer_id),
					Err(e) => println!("Failed to send pairing request to {}: {}", peer_id, e),
				}
			}
		}
		
		// 2. Query DHT for remote pairing (primary method when mDNS fails)
		let key = libp2p::kad::RecordKey::new(&session_id.as_bytes());
		let query_id = service.query_dht_record(key).await?;
		println!("🔍 Querying DHT for pairing session: session={}, query_id={:?}", session_id, query_id);
		
		// 3. Add periodic DHT queries as backup in case the first query fails
		// This is important for test environments where mDNS might not work
		let networking_ref = networking.clone();
		let session_id_clone = session_id;
		tokio::spawn(async move {
			for i in 1..=3 {
				tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
				let key = libp2p::kad::RecordKey::new(&session_id_clone.as_bytes());
				let service = networking_ref.read().await;
				match service.query_dht_record(key).await {
					Ok(query_id) => {
						println!("🔍 DHT Retry {}: Querying for session {} (query_id: {:?})", i, session_id_clone, query_id);
					}
					Err(e) => {
						println!("⚠️ DHT Retry {}: Failed to query session {}: {}", i, session_id_clone, e);
					}
				}
			}
		});

		Ok(())
	}

	/// Get current pairing status
	pub async fn get_pairing_status(
		&self,
	) -> Result<Vec<networking::PairingSession>, Box<dyn std::error::Error>> {
		let networking = self
			.networking
			.as_ref()
			.ok_or("Networking not initialized. Call init_networking() first.")?;

		// Get pairing handler from protocol registry
		let service = networking.read().await;
		let registry = service.protocol_registry();
		let pairing_handler = registry
			.read()
			.await
			.get_handler("pairing")
			.ok_or("Pairing protocol not registered")?;

		// Downcast to concrete pairing handler type to access sessions
		if let Some(pairing_handler) = pairing_handler.as_any().downcast_ref::<PairingProtocolHandler>() {
			let sessions = pairing_handler.get_active_sessions().await;
			Ok(sessions)
		} else {
			Err("Failed to downcast pairing handler".into())
		}
	}

	/// List pending pairing requests (converted from active pairing sessions)
	pub async fn list_pending_pairings(
		&self,
	) -> Result<Vec<PendingPairingRequest>, Box<dyn std::error::Error>> {
		let sessions = self.get_pairing_status().await?;

		// Convert active pairing sessions to pending requests
		let pending_requests: Vec<PendingPairingRequest> = sessions
			.into_iter()
			.filter(|session| {
				matches!(
					session.state,
					networking::PairingState::WaitingForConnection
				)
			})
			.map(|session| PendingPairingRequest {
				request_id: session.id,
				device_id: session.remote_device_id.unwrap_or(session.id),
				device_name: "Unknown Device".to_string(),
				received_at: session.created_at,
			})
			.collect();

		Ok(pending_requests)
	}

	/// Accept a pairing request (cancel pairing session if rejecting)
	pub async fn accept_pairing_request(
		&self,
		request_id: uuid::Uuid,
	) -> Result<(), Box<dyn std::error::Error>> {
		// Pairing acceptance is handled automatically in the new system
		info!(
			"Accepting pairing request: {} (handled automatically)",
			request_id
		);
		Ok(())
	}

	/// Reject a pairing request (cancel the pairing session)
	pub async fn reject_pairing_request(
		&self,
		request_id: uuid::Uuid,
	) -> Result<(), Box<dyn std::error::Error>> {
		let networking = self
			.networking
			.as_ref()
			.ok_or("Networking not initialized. Call init_networking() first.")?;

		// Get pairing handler and cancel session
		let service = networking.read().await;
		let registry = service.protocol_registry();
		let _pairing_handler = registry
			.read()
			.await
			.get_handler("pairing")
			.ok_or("Pairing protocol not registered")?;

		// For now, just log - in full implementation we'd cancel the session
		info!("Rejected pairing request: {}", request_id);
		Ok(())
	}

	/// Get network identity for subprocess helper
	pub async fn get_network_identity(
		&self,
	) -> Result<networking::NetworkIdentity, Box<dyn std::error::Error>> {
		let networking = self
			.networking
			.as_ref()
			.ok_or("Networking not initialized. Call init_networking() first.")?;

		let service = networking.read().await;
		Ok(service.identity().clone())
	}
}
