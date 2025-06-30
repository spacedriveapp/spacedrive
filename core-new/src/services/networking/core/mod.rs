//! Core networking engine with unified LibP2P swarm

pub mod behavior;
pub mod discovery;
pub mod event_loop;
pub mod swarm;

use crate::device::DeviceManager;
use crate::services::networking::{
	device::{DeviceInfo, DeviceRegistry},
	protocols::{pairing::PairingProtocolHandler, ProtocolRegistry},
	utils::{logging::ConsoleLogger, NetworkIdentity},
	NetworkingError, Result,
};
use libp2p::{
	kad::{QueryId, RecordKey},
	Multiaddr, PeerId, Swarm,
};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

pub use behavior::{UnifiedBehaviour, UnifiedBehaviourEvent};
pub use event_loop::{EventLoopCommand, NetworkingEventLoop};

/// Central networking event types
#[derive(Debug, Clone)]
pub enum NetworkEvent {
	// Discovery events
	PeerDiscovered {
		peer_id: PeerId,
		addresses: Vec<Multiaddr>,
	},
	PeerDisconnected {
		peer_id: PeerId,
	},

	// Pairing events
	PairingRequest {
		session_id: Uuid,
		device_info: DeviceInfo,
		peer_id: PeerId,
	},
	PairingSessionDiscovered {
		session_id: Uuid,
		peer_id: PeerId,
		addresses: Vec<Multiaddr>,
		device_info: DeviceInfo,
	},
	PairingCompleted {
		device_id: Uuid,
		device_info: DeviceInfo,
	},
	PairingFailed {
		session_id: Uuid,
		reason: String,
	},

	// Connection events
	ConnectionEstablished {
		device_id: Uuid,
		peer_id: PeerId,
	},
	ConnectionLost {
		device_id: Uuid,
		peer_id: PeerId,
	},
	MessageReceived {
		from: Uuid,
		protocol: String,
		data: Vec<u8>,
	},
}

/// Main networking service - single source of truth for all networking operations
pub struct NetworkingService {
	/// Our network identity
	identity: NetworkIdentity,

	/// LibP2P swarm with unified behavior
	swarm: Swarm<UnifiedBehaviour>,

	/// Shutdown sender for stopping the event loop
	shutdown_sender: Arc<RwLock<Option<mpsc::UnboundedSender<()>>>>,

	/// Command sender for sending commands to the event loop
	command_sender: Option<mpsc::UnboundedSender<event_loop::EventLoopCommand>>,

	/// Registry for protocol handlers
	protocol_registry: Arc<RwLock<ProtocolRegistry>>,

	/// Registry for device state and connections
	device_registry: Arc<RwLock<DeviceRegistry>>,

	/// Event sender for broadcasting network events
	event_sender: mpsc::UnboundedSender<NetworkEvent>,

	/// Event receiver for subscribers
	event_receiver: Arc<RwLock<Option<mpsc::UnboundedReceiver<NetworkEvent>>>>,
}

impl NetworkingService {
	/// Create a new networking core
	pub async fn new(
		device_manager: Arc<DeviceManager>,
		library_key_manager: Arc<crate::keys::library_key_manager::LibraryKeyManager>,
		data_dir: impl AsRef<std::path::Path>,
	) -> Result<Self> {
		// Generate network identity from master key
		let device_key = device_manager
			.master_key()
			.map_err(|e| NetworkingError::Protocol(format!("Failed to get device key: {}", e)))?;
		let identity = NetworkIdentity::from_device_key(&device_key).await?;

		// Create LibP2P swarm
		let swarm = swarm::create_swarm(identity.clone()).await?;

		// Create event channel
		let (event_sender, event_receiver) = mpsc::unbounded_channel();

		// Create registries
		let protocol_registry = Arc::new(RwLock::new(ProtocolRegistry::new()));
		let device_registry = Arc::new(RwLock::new(DeviceRegistry::new(device_manager, data_dir)?));

		// Note: Protocol handlers will be registered by the Core during init_networking
		// to avoid duplicate registrations

		Ok(Self {
			identity,
			swarm,
			shutdown_sender: Arc::new(RwLock::new(None)),
			command_sender: None,
			protocol_registry,
			device_registry,
			event_sender,
			event_receiver: Arc::new(RwLock::new(Some(event_receiver))),
		})
	}

	/// Start the networking service
	pub async fn start(&mut self) -> Result<()> {
		// Start LibP2P listeners (TCP-only to match simplified transport)
		self.swarm
			.listen_on("/ip4/0.0.0.0/tcp/0".parse().unwrap())
			.map_err(|e| NetworkingError::Transport(e.to_string()))?;
		// Removed QUIC UDP listener to match TCP-only transport configuration

		// Create and start event loop by moving the swarm
		let swarm = std::mem::replace(&mut self.swarm, {
			// Create a new swarm for the replace (this won't be used)
			swarm::create_swarm(self.identity.clone()).await?
		});

		let event_loop = NetworkingEventLoop::new(
			swarm,
			self.protocol_registry.clone(),
			self.device_registry.clone(),
			self.event_sender.clone(),
			self.identity.clone(),
		);

		// Store shutdown and command senders before starting
		let shutdown_sender = event_loop.shutdown_sender();
		let command_sender = event_loop.command_sender();

		// Start the event processing in background (consumes event_loop)
		event_loop.start().await?;

		// Store senders for later use
		*self.shutdown_sender.write().await = Some(shutdown_sender);
		self.command_sender = Some(command_sender);

		// Load and attempt to reconnect to paired devices
		self.load_and_reconnect_devices().await?;

		// Start periodic reconnection attempts
		self.start_periodic_reconnection().await;

		Ok(())
	}

	/// Load paired devices from persistence and attempt reconnection
	async fn load_and_reconnect_devices(&mut self) -> Result<()> {
		let mut device_registry = self.device_registry.write().await;

		// Load paired devices from persistence
		let loaded_device_ids = device_registry.load_paired_devices().await?;
		println!(
			"📱 Loaded {} paired devices from persistence",
			loaded_device_ids.len()
		);

		// Get devices that should auto-reconnect
		let auto_reconnect_devices = device_registry.get_auto_reconnect_devices().await?;
		println!(
			"🔄 Found {} devices for auto-reconnection",
			auto_reconnect_devices.len()
		);

		drop(device_registry); // Release the lock for async operations

		// Start background reconnection attempts
		self.start_background_reconnection(auto_reconnect_devices)
			.await;

		Ok(())
	}

	/// Start background reconnection attempts for paired devices
	async fn start_background_reconnection(
		&self,
		auto_reconnect_devices: Vec<(
			Uuid,
			crate::services::networking::device::PersistedPairedDevice,
		)>,
	) {
		for (device_id, persisted_device) in auto_reconnect_devices {
			let command_sender = self.command_sender.clone();

			// Spawn a background task for each device reconnection
			tokio::spawn(async move {
				Self::attempt_device_reconnection(device_id, persisted_device, command_sender)
					.await;
			});
		}
	}

	/// Attempt to reconnect to a specific device
	async fn attempt_device_reconnection(
		device_id: Uuid,
		persisted_device: crate::services::networking::device::PersistedPairedDevice,
		command_sender: Option<tokio::sync::mpsc::UnboundedSender<EventLoopCommand>>,
	) {
		println!(
			"🔄 Starting reconnection attempts for device: {}",
			device_id
		);

		// Try each known address with exponential backoff
		let mut attempt = 0;
		let max_attempts = 3;

		for address_str in &persisted_device.last_seen_addresses {
			if attempt >= max_attempts {
				break;
			}

			if let Ok(multiaddr) = address_str.parse::<libp2p::Multiaddr>() {
				println!(
					"🔍 Attempt {}: Connecting to device {} at {}",
					attempt + 1,
					device_id,
					multiaddr
				);

				if let Some(ref sender) = command_sender {
					// Extract peer ID from the persisted device info
					if let Ok(peer_id) = persisted_device
						.device_info
						.network_fingerprint
						.peer_id
						.parse::<PeerId>()
					{
						// Create a one-shot channel for the response
						let (response_tx, response_rx) = tokio::sync::oneshot::channel();

						// Send dial command to the networking event loop
						let dial_command = EventLoopCommand::DialPeer {
							peer_id,
							address: multiaddr.clone(),
							response_channel: response_tx,
						};

						if let Err(e) = sender.send(dial_command) {
							eprintln!(
								"Failed to send dial command for device {}: {}",
								device_id, e
							);
							continue;
						}

						// Wait for the dial result
						match tokio::time::timeout(
							tokio::time::Duration::from_secs(10),
							response_rx,
						)
						.await
						{
							Ok(Ok(Ok(()))) => {
								println!(
									"✅ Successfully connected to device {} at {}",
									device_id, multiaddr
								);
								return; // Success, exit reconnection attempts
							}
							Ok(Ok(Err(e))) => {
								println!(
									"❌ Failed to connect to device {} at {}: {}",
									device_id, multiaddr, e
								);
							}
							Ok(Err(_)) => {
								println!(
									"❌ Channel closed while connecting to device {} at {}",
									device_id, multiaddr
								);
							}
							Err(_) => {
								println!(
									"⏰ Timeout connecting to device {} at {}",
									device_id, multiaddr
								);
							}
						}
					} else {
						eprintln!(
							"⚠️ Invalid peer ID for device {}: {}",
							device_id, persisted_device.device_info.network_fingerprint.peer_id
						);
					}

					// Wait before next attempt with exponential backoff
					let delay = tokio::time::Duration::from_secs(2_u64.pow(attempt as u32));
					tokio::time::sleep(delay).await;
				}
			}

			attempt += 1;
		}

		if attempt >= max_attempts {
			println!(
				"⚠️ Failed to reconnect to device {} after {} attempts",
				device_id, max_attempts
			);
		}
	}

	/// Start periodic reconnection attempts for disconnected devices
	async fn start_periodic_reconnection(&self) {
		let device_registry = self.device_registry.clone();
		let command_sender = self.command_sender.clone();

		tokio::spawn(async move {
			let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30)); // Check every 30 seconds

			loop {
				interval.tick().await;

				// Get disconnected devices that should be reconnected
				if let Ok(auto_reconnect_devices) = {
					let registry = device_registry.read().await;
					registry.get_auto_reconnect_devices().await
				} {
					// Only attempt reconnection for devices we haven't seen recently
					let now = chrono::Utc::now();
					for (device_id, persisted_device) in auto_reconnect_devices {
						// Skip if device was seen recently (within last 5 minutes)
						if let Some(last_connected) = persisted_device.last_connected_at {
							if now.signed_duration_since(last_connected)
								< chrono::Duration::minutes(5)
							{
								continue;
							}
						}

						// Check if device is currently disconnected in registry
						let is_disconnected = {
							let registry = device_registry.read().await;
							if let Some(device_state) = registry.get_device_state(device_id) {
								matches!(device_state, crate::services::networking::device::DeviceState::Disconnected { .. })
							} else {
								true // Not in registry, try to reconnect
							}
						};

						if is_disconnected {
							println!(
								"🔄 Attempting periodic reconnection to device: {}",
								device_id
							);
							let cmd_sender = command_sender.clone();
							tokio::spawn(async move {
								Self::attempt_device_reconnection(
									device_id,
									persisted_device,
									cmd_sender,
								)
								.await;
							});
						}
					}
				}
			}
		});
	}

	/// Stop the networking service
	pub async fn shutdown(&self) -> Result<()> {
		if let Some(shutdown_sender) = self.shutdown_sender.write().await.take() {
			let _ = shutdown_sender.send(());
			// Wait a bit for graceful shutdown
			tokio::time::sleep(std::time::Duration::from_millis(100)).await;
		}
		Ok(())
	}

	/// Subscribe to network events
	pub async fn subscribe_events(&self) -> Option<mpsc::UnboundedReceiver<NetworkEvent>> {
		self.event_receiver.write().await.take()
	}

	/// Get our network identity
	pub fn identity(&self) -> &NetworkIdentity {
		&self.identity
	}

	/// Get our peer ID
	pub fn peer_id(&self) -> PeerId {
		self.identity.peer_id()
	}

	/// Get connected devices
	pub async fn get_connected_devices(&self) -> Vec<DeviceInfo> {
		self.device_registry.read().await.get_connected_devices()
	}

	/// Get raw connected peers directly from swarm (bypasses DeviceRegistry)
	/// This is critical for handling the race condition where connections exist
	/// but devices haven't been registered yet
	pub async fn get_raw_connected_peers(&self) -> Vec<PeerId> {
		if let Some(command_sender) = &self.command_sender {
			let (response_tx, response_rx) = tokio::sync::oneshot::channel();
			let command = event_loop::EventLoopCommand::GetRawConnectedPeers {
				response_channel: response_tx,
			};

			if command_sender.send(command).is_ok() {
				if let Ok(peers) = response_rx.await {
					return peers;
				}
			}
		}
		Vec::new()
	}

	/// Send a message to a device
	pub async fn send_message(&self, device_id: Uuid, protocol: &str, data: Vec<u8>) -> Result<()> {
		if let Some(command_sender) = &self.command_sender {
			let command = event_loop::EventLoopCommand::SendMessage {
				device_id,
				protocol: protocol.to_string(),
				data,
			};

			command_sender.send(command).map_err(|_| {
				NetworkingError::ConnectionFailed("Event loop not running".to_string())
			})?;

			Ok(())
		} else {
			Err(NetworkingError::ConnectionFailed(
				"Networking not started".to_string(),
			))
		}
	}

	/// Get protocol registry for registering new protocols
	pub fn protocol_registry(&self) -> Arc<RwLock<ProtocolRegistry>> {
		self.protocol_registry.clone()
	}

	/// Get device registry for device management
	pub fn device_registry(&self) -> Arc<RwLock<DeviceRegistry>> {
		self.device_registry.clone()
	}

	/// Publish a DHT record for pairing session discovery
	pub async fn publish_dht_record(&self, key: RecordKey, value: Vec<u8>) -> Result<QueryId> {
		if let Some(command_sender) = &self.command_sender {
			let (response_tx, response_rx) = tokio::sync::oneshot::channel();
			let command = event_loop::EventLoopCommand::PublishDhtRecord {
				key,
				value,
				response_channel: response_tx,
			};

			command_sender.send(command).map_err(|_| {
				NetworkingError::ConnectionFailed("Event loop not running".to_string())
			})?;

			response_rx.await.map_err(|_| {
				NetworkingError::ConnectionFailed("Failed to receive DHT response".to_string())
			})?
		} else {
			Err(NetworkingError::ConnectionFailed(
				"Networking not started".to_string(),
			))
		}
	}

	/// Query a DHT record for pairing session discovery
	pub async fn query_dht_record(&self, key: RecordKey) -> Result<QueryId> {
		if let Some(command_sender) = &self.command_sender {
			let (response_tx, response_rx) = tokio::sync::oneshot::channel();
			let command = event_loop::EventLoopCommand::QueryDhtRecord {
				key,
				response_channel: response_tx,
			};

			command_sender.send(command).map_err(|_| {
				NetworkingError::ConnectionFailed("Event loop not running".to_string())
			})?;

			response_rx.await.map_err(|_| {
				NetworkingError::ConnectionFailed("Failed to receive DHT response".to_string())
			})?
		} else {
			Err(NetworkingError::ConnectionFailed(
				"Networking not started".to_string(),
			))
		}
	}

	/// Get currently connected peers for direct pairing attempts
	pub async fn get_connected_peers(&self) -> Vec<PeerId> {
		// Get connected peers from device registry
		let registry = self.device_registry.read().await;
		registry.get_connected_peers()
	}

	/// Get the local device ID
	pub fn device_id(&self) -> Uuid {
		self.identity.device_id()
	}

	/// Get the command sender for the event loop
	pub fn command_sender(&self) -> Option<&mpsc::UnboundedSender<event_loop::EventLoopCommand>> {
		self.command_sender.as_ref()
	}

	/// Send message to a specific peer ID (bypassing device lookup)
	pub async fn send_message_to_peer(
		&self,
		peer_id: PeerId,
		protocol: &str,
		data: Vec<u8>,
	) -> Result<()> {
		if let Some(command_sender) = &self.command_sender {
			let command = event_loop::EventLoopCommand::SendMessageToPeer {
				peer_id,
				protocol: protocol.to_string(),
				data,
			};

			command_sender.send(command).map_err(|_| {
				NetworkingError::ConnectionFailed("Event loop not running".to_string())
			})?;

			Ok(())
		} else {
			Err(NetworkingError::ConnectionFailed(
				"Networking not started".to_string(),
			))
		}
	}

	/// Dial a peer at a specific address
	pub async fn dial_peer(&self, peer_id: PeerId, address: Multiaddr) -> Result<()> {
		if let Some(command_sender) = &self.command_sender {
			let (response_tx, response_rx) = tokio::sync::oneshot::channel();
			let command = event_loop::EventLoopCommand::DialPeer {
				peer_id,
				address: address.clone(),
				response_channel: response_tx,
			};

			command_sender.send(command).map_err(|_| {
				NetworkingError::ConnectionFailed("Event loop not running".to_string())
			})?;

			response_rx.await.map_err(|_| {
				NetworkingError::ConnectionFailed("Failed to receive dial response".to_string())
			})?
		} else {
			Err(NetworkingError::ConnectionFailed(
				"Networking not started".to_string(),
			))
		}
	}

	/// Get external addresses for advertising in DHT records
	pub async fn get_external_addresses(&self) -> Vec<Multiaddr> {
		// Query the event loop for current listening addresses
		if let Some(command_sender) = &self.command_sender {
			// Retry a few times to allow swarm to establish listeners
			for attempt in 1..=3 {
				let (response_tx, response_rx) = tokio::sync::oneshot::channel();

				let command = event_loop::EventLoopCommand::GetListeningAddresses {
					response_channel: response_tx,
				};

				if let Err(e) = command_sender.send(command) {
					eprintln!("Failed to send GetListeningAddresses command: {}", e);
					return Vec::new();
				}

				match response_rx.await {
					Ok(addresses) => {
						if addresses.is_empty() {
							if attempt < 3 {
								println!(
									"No external addresses found on attempt {}, retrying...",
									attempt
								);
								tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
								continue;
							} else {
								eprintln!("No external addresses found after {} attempts", attempt);
								return Vec::new();
							}
						} else {
							println!("Found external addresses for advertising: {:?}", addresses);
							return addresses;
						}
					}
					Err(e) => {
						eprintln!("Failed to receive listening addresses: {}", e);
						return Vec::new();
					}
				}
			}
			Vec::new()
		} else {
			eprintln!("Event loop not started, cannot get listening addresses");
			Vec::new()
		}
	}

	/// Start pairing as an initiator (generates pairing code)
	pub async fn start_pairing_as_initiator(&self) -> Result<(String, u32)> {
		// Get pairing handler from protocol registry
		let registry = self.protocol_registry();
		let pairing_handler =
			registry
				.read()
				.await
				.get_handler("pairing")
				.ok_or(NetworkingError::Protocol(
					"Pairing protocol not registered".to_string(),
				))?;

		// Cast to pairing handler to access pairing-specific methods
		let pairing_handler = pairing_handler
			.as_any()
			.downcast_ref::<crate::services::networking::protocols::PairingProtocolHandler>()
			.ok_or(NetworkingError::Protocol(
				"Invalid pairing handler type".to_string(),
			))?;

		// Generate session ID first as canonical source of truth
		let session_id = uuid::Uuid::new_v4();
		let pairing_code =
			crate::services::networking::protocols::pairing::PairingCode::from_session_id(
				session_id,
			);

		// Start pairing session with the session ID and pairing code
		pairing_handler
			.start_pairing_session_with_id(session_id, pairing_code.clone())
			.await?;

		// CRITICAL FIX: Register Alice in the device registry with the session mapping
		// This creates the session_id → device_id mapping that Bob needs to find Alice
		let alice_device_id = self.device_id();
		let alice_peer_id = self.peer_id();
		let device_registry = self.device_registry();
		{
			let mut registry = device_registry.write().await;
			registry.start_pairing(alice_device_id, alice_peer_id, session_id)?;
		}

		// Get external addresses for advertising
		let external_addresses = self.get_external_addresses().await;
		let address_strings: Vec<String> = external_addresses
			.into_iter()
			.map(|addr| addr.to_string())
			.collect();

		if address_strings.is_empty() {
			return Err(NetworkingError::Protocol(
				"No external addresses available for advertising - pairing cannot start"
					.to_string(),
			));
		}

		// Create pairing advertisement for DHT
		let advertisement = crate::services::networking::protocols::pairing::PairingAdvertisement {
			peer_id: self.peer_id().to_string(),
			addresses: address_strings,
			device_info: pairing_handler.get_device_info().await?,
			expires_at: chrono::Utc::now() + chrono::Duration::minutes(5),
			created_at: chrono::Utc::now(),
		};

		// CRITICAL FIX: Use actual session ID for DHT key (not pairing code session ID)
		let key = libp2p::kad::RecordKey::new(&session_id.as_bytes());
		let value = serde_json::to_vec(&advertisement)
			.map_err(|e| NetworkingError::Protocol(e.to_string()))?;

		let _query_id = self.publish_dht_record(key, value).await?;

		let expires_in = 300; // 5 minutes

		Ok((pairing_code.to_string(), expires_in))
	}

	/// Start pairing as a joiner (connects using pairing code)
	pub async fn start_pairing_as_joiner(&self, code: &str) -> Result<()> {
		// Parse BIP39 pairing code
		let pairing_code =
			crate::services::networking::protocols::pairing::PairingCode::from_string(code)?;
		let session_id = pairing_code.session_id();

		// CRITICAL FIX: Join Alice's pairing session using her session ID
		let registry = self.protocol_registry();
		let pairing_handler =
			registry
				.read()
				.await
				.get_handler("pairing")
				.ok_or(NetworkingError::Protocol(
					"Pairing protocol not registered".to_string(),
				))?;
		let pairing_handler = pairing_handler
			.as_any()
			.downcast_ref::<crate::services::networking::protocols::PairingProtocolHandler>()
			.ok_or(NetworkingError::Protocol(
				"Invalid pairing handler type".to_string(),
			))?;

		// Join Alice's pairing session using the session ID and pairing code
		pairing_handler
			.join_pairing_session(session_id, pairing_code)
			.await?;

		// Unified Pairing Flow: Support both mDNS (local) and DHT (remote) simultaneously
		// Both methods run in parallel, first successful response completes pairing

		// Method 1: DHT-based remote pairing (for cross-network scenarios)
		// Query DHT for Alice's published session record - THIS MUST BE FIRST
		let key = libp2p::kad::RecordKey::new(&session_id.as_bytes());
		let _ = self.query_dht_record(key).await;

		// Method 2: mDNS-based local pairing (already handled by event loop)
		// The event loop automatically detects mDNS peers and schedules pairing requests
		// This handles Alice and Bob on the same network

		// Method 3: Wait for connections to be established and ensure pairing requests are sent
		self.ensure_pairing_requests_sent(session_id).await?;

		// Method 4: Direct requests to any currently connected peers (immediate attempt)
		// This covers cases where Alice is already connected but not yet paired
		let connected_peers = self.get_connected_peers().await;
		if !connected_peers.is_empty() {
			for peer_id in connected_peers {
				// Get local device info for the joiner
				let local_device_info = {
					let device_registry = self.device_registry();
					let registry = device_registry.read().await;
					registry.get_local_device_info().unwrap_or_else(|_| {
						crate::services::networking::device::DeviceInfo {
							device_id: self.device_id(),
							device_name: "Joiner Device".to_string(),
							device_type: crate::services::networking::device::DeviceType::Desktop,
							os_version: std::env::consts::OS.to_string(),
							app_version: env!("CARGO_PKG_VERSION").to_string(),
							network_fingerprint: self.identity().network_fingerprint(),
							last_seen: chrono::Utc::now(),
						}
					})
				};

				let pairing_request =
					crate::services::networking::core::behavior::PairingMessage::PairingRequest {
						session_id,
						device_info: local_device_info,
						public_key: self.identity().public_key_bytes(),
					};

				let _ = self
					.send_message_to_peer(
						peer_id,
						"pairing",
						serde_json::to_vec(&pairing_request).unwrap_or_default(),
					)
					.await;
			}
		}

		// Add periodic DHT retries for reliability in challenging network conditions
		let command_sender = self.command_sender.clone();
		let session_id_clone = session_id;
		if let Some(command_sender) = command_sender {
			tokio::spawn(async move {
				for _i in 1..=3 {
					tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
					let key = libp2p::kad::RecordKey::new(&session_id_clone.as_bytes());
					let (response_tx, _response_rx) = tokio::sync::oneshot::channel();
					let command = event_loop::EventLoopCommand::QueryDhtRecord {
						key,
						response_channel: response_tx,
					};
					let _ = command_sender.send(command);
				}
			});
		}

		Ok(())
	}

	/// Get current pairing status
	pub async fn get_pairing_status(
		&self,
	) -> Result<Vec<crate::services::networking::PairingSession>> {
		// Get pairing handler from protocol registry
		let registry = self.protocol_registry();
		let pairing_handler =
			registry
				.read()
				.await
				.get_handler("pairing")
				.ok_or(NetworkingError::Protocol(
					"Pairing protocol not registered".to_string(),
				))?;

		// Downcast to concrete pairing handler type to access sessions
		if let Some(pairing_handler) =
			pairing_handler
				.as_any()
				.downcast_ref::<crate::services::networking::protocols::PairingProtocolHandler>()
		{
			let sessions = pairing_handler.get_active_sessions().await;
			Ok(sessions)
		} else {
			Err(NetworkingError::Protocol(
				"Failed to downcast pairing handler".to_string(),
			))
		}
	}

	/// Enhanced pairing request sending with robust active polling
	/// This method continuously checks for connections and ensures requests are sent
	/// regardless of when the connection was established (fixes race condition)
	async fn ensure_pairing_requests_sent(&self, session_id: uuid::Uuid) -> Result<()> {
		const MAX_WAIT_TIME: u64 = 15000; // 15 seconds
		const POLL_INTERVAL: u64 = 500; // Check every 500ms
		let start_time = std::time::Instant::now();

		loop {
			// First, check if the session has already advanced. If so, our job is done.
			let registry = self.protocol_registry();
			let registry_guard = registry.read().await;
			if let Some(pairing_handler) = registry_guard.get_handler("pairing") {
				if let Some(handler) =
					pairing_handler
						.as_any()
						.downcast_ref::<crate::services::networking::protocols::PairingProtocolHandler>(
					) {
					let sessions = handler.get_active_sessions().await;
					if let Some(session) = sessions.iter().find(|s| s.id == session_id) {
						if !matches!(
							session.state,
							crate::services::networking::protocols::pairing::PairingState::Scanning
						) {
							return Ok(());
						}
					}
				}
			}
			drop(registry_guard);

			// If not, actively check for connected peers and send the request.
			// CRITICAL: Use raw swarm connections, not DeviceRegistry (fixes Catch-22)
			let connected_peers = self.get_raw_connected_peers().await;
			if !connected_peers.is_empty() {
				for peer_id in &connected_peers {
					let local_device_info = {
						let device_registry = self.device_registry();
						let registry = device_registry.read().await;
						registry.get_local_device_info().unwrap_or_else(|_| {
							crate::services::networking::device::DeviceInfo {
								device_id: self.device_id(),
								device_name: "Bob's Test Device".to_string(),
								device_type:
									crate::services::networking::device::DeviceType::Desktop,
								os_version: std::env::consts::OS.to_string(),
								app_version: env!("CARGO_PKG_VERSION").to_string(),
								network_fingerprint: self.identity().network_fingerprint(),
								last_seen: chrono::Utc::now(),
							}
						})
					};

					let pairing_request =
						crate::services::networking::core::behavior::PairingMessage::PairingRequest {
							session_id,
							device_info: local_device_info,
							public_key: self.identity().public_key_bytes(),
						};

					// This is an idempotent action; sending the request multiple times is okay
					// as Alice's handler will just re-send the same challenge.
					let _ = self
						.send_message_to_peer(
							*peer_id,
							"pairing",
							serde_json::to_vec(&pairing_request).unwrap_or_default(),
						)
						.await;
				}
			}

			// Check for timeout
			if start_time.elapsed().as_millis() > MAX_WAIT_TIME as u128 {
				return Err(NetworkingError::Protocol(
					"Pairing timeout: Did not receive challenge from Alice.".to_string(),
				));
			}

			tokio::time::sleep(tokio::time::Duration::from_millis(POLL_INTERVAL)).await;
		}
	}
}

// Ensure NetworkingService is Send + Sync for proper async usage
unsafe impl Send for NetworkingService {}
unsafe impl Sync for NetworkingService {}
