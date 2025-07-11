//! Core networking engine with Iroh P2P

pub mod event_loop;

use crate::device::DeviceManager;
use crate::services::networking::{
	device::{DeviceInfo, DeviceRegistry},
	protocols::{pairing::PairingProtocolHandler, ProtocolRegistry},
	utils::{NetworkIdentity, logging::NetworkLogger},
	NetworkingError, Result,
};
use iroh::net::endpoint::Connection;
use iroh::net::key::NodeId;
use iroh::net::{Endpoint, NodeAddr};
use iroh_net::discovery::local_swarm_discovery::LocalSwarmDiscovery;
use iroh_net::discovery::{ConcurrentDiscovery, Discovery};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

pub use event_loop::{EventLoopCommand, NetworkingEventLoop};

/// Protocol ALPN identifiers
pub const PAIRING_ALPN: &[u8] = b"spacedrive/pairing/1";
pub const FILE_TRANSFER_ALPN: &[u8] = b"spacedrive/filetransfer/1";
pub const MESSAGING_ALPN: &[u8] = b"spacedrive/messaging/1";

/// Central networking event types
#[derive(Debug, Clone)]
pub enum NetworkEvent {
	// Discovery events
	PeerDiscovered {
		node_id: NodeId,
		node_addr: NodeAddr,
	},
	PeerDisconnected {
		node_id: NodeId,
	},

	// Pairing events
	PairingRequest {
		session_id: Uuid,
		device_info: DeviceInfo,
		node_id: NodeId,
	},
	PairingSessionDiscovered {
		session_id: Uuid,
		node_id: NodeId,
		node_addr: NodeAddr,
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
		node_id: NodeId,
	},
	ConnectionLost {
		device_id: Uuid,
		node_id: NodeId,
	},
	MessageReceived {
		from: Uuid,
		protocol: String,
		data: Vec<u8>,
	},
}

/// Main networking service using Iroh
pub struct NetworkingService {
	/// Iroh endpoint for all networking
	endpoint: Option<Endpoint>,

	/// Our network identity
	identity: NetworkIdentity,

	/// Our Iroh node ID
	node_id: NodeId,

	/// Discovery service for finding peers
	discovery: Option<Box<dyn Discovery>>,

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

	/// Active connections tracker
	active_connections: Arc<RwLock<std::collections::HashMap<NodeId, Connection>>>,

	/// Logger for networking operations
	logger: Arc<dyn NetworkLogger>,
}

impl NetworkingService {
	/// Create a new networking service
	pub async fn new(
		device_manager: Arc<DeviceManager>,
		library_key_manager: Arc<crate::keys::library_key_manager::LibraryKeyManager>,
		data_dir: impl AsRef<std::path::Path>,
		logger: Arc<dyn NetworkLogger>,
	) -> Result<Self> {
		// Generate network identity from master key
		let device_key = device_manager
			.master_key()
			.map_err(|e| NetworkingError::Protocol(format!("Failed to get device key: {}", e)))?;
		let identity = NetworkIdentity::from_device_key(&device_key).await?;

		// Convert identity to Iroh format
		let secret_key = identity.to_iroh_secret_key()?;
		let node_id = secret_key.public();

		// Create event channel
		let (event_sender, event_receiver) = mpsc::unbounded_channel();

		// Create registries
		let protocol_registry = Arc::new(RwLock::new(ProtocolRegistry::new()));
		let device_registry = Arc::new(RwLock::new(DeviceRegistry::new(device_manager, data_dir)?));

		Ok(Self {
			endpoint: None,
			identity,
			node_id,
			discovery: None,
			shutdown_sender: Arc::new(RwLock::new(None)),
			command_sender: None,
			protocol_registry,
			device_registry,
			event_sender,
			event_receiver: Arc::new(RwLock::new(Some(event_receiver))),
			active_connections: Arc::new(RwLock::new(std::collections::HashMap::new())),
			logger,
		})
	}

	/// Start the networking service
	pub async fn start(&mut self) -> Result<()> {
		// Create Iroh endpoint with discovery and relay configuration
		let secret_key = self.identity.to_iroh_secret_key()?;

		// Create discovery service - using local swarm discovery for now
		let discovery = LocalSwarmDiscovery::new(self.node_id).map_err(|e| {
			NetworkingError::Transport(format!("Failed to create discovery: {}", e))
		})?;

		// Create endpoint with discovery
		let endpoint = Endpoint::builder()
			.secret_key(secret_key)
			.alpns(vec![
				PAIRING_ALPN.to_vec(),
				FILE_TRANSFER_ALPN.to_vec(),
				MESSAGING_ALPN.to_vec(),
			])
			.relay_mode(iroh_net::relay::RelayMode::Default)
			.discovery(Box::new(discovery))
			.bind_addr_v4(std::net::SocketAddrV4::new(
				std::net::Ipv4Addr::UNSPECIFIED,
				0,
			))
			.bind_addr_v6(std::net::SocketAddrV6::new(
				std::net::Ipv6Addr::UNSPECIFIED,
				0,
				0,
				0,
			))
			.bind()
			.await
			.map_err(|e| NetworkingError::Transport(format!("Failed to create endpoint: {}", e)))?;

		// Store endpoint reference for other methods
		self.endpoint = Some(endpoint.clone());

		// Create and start event loop
		let event_loop = NetworkingEventLoop::new(
			endpoint,
			self.protocol_registry.clone(),
			self.device_registry.clone(),
			self.event_sender.clone(),
			self.identity.clone(),
			self.active_connections.clone(),
		);

		// Store shutdown and command senders before starting
		let shutdown_sender = event_loop.shutdown_sender();
		let command_sender = event_loop.command_sender();

		// Start the event processing in background
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
		self.logger
			.info(&format!(
				"📱 Loaded {} paired devices from persistence",
				loaded_device_ids.len()
			))
			.await;

		// Get devices that should auto-reconnect
		let auto_reconnect_devices = device_registry.get_auto_reconnect_devices().await?;
		self.logger
			.info(&format!(
				"🔄 Found {} devices for auto-reconnection",
				auto_reconnect_devices.len()
			))
			.await;

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
			let endpoint = self.endpoint.clone();
			let logger = self.logger.clone();

			// Spawn a background task for each device reconnection
			tokio::spawn(async move {
				Self::attempt_device_reconnection(
					device_id,
					persisted_device,
					command_sender,
					endpoint,
					logger,
				)
				.await;
			});
		}
	}

	/// Attempt to reconnect to a specific device
	async fn attempt_device_reconnection(
		device_id: Uuid,
		persisted_device: crate::services::networking::device::PersistedPairedDevice,
		command_sender: Option<tokio::sync::mpsc::UnboundedSender<EventLoopCommand>>,
		endpoint: Option<Endpoint>,
		logger: Arc<dyn NetworkLogger>,
	) {
		logger
			.info(&format!(
				"🔄 Starting reconnection attempts for device: {}",
				device_id
			))
			.await;

		if let (Some(endpoint), Some(sender)) = (endpoint, command_sender) {
			// Try to parse node ID from the persisted device
			if let Ok(node_id) = persisted_device
				.device_info
				.network_fingerprint
				.node_id
				.parse::<NodeId>()
			{
				// Build NodeAddr from persisted addresses
				let mut node_addr = NodeAddr::new(node_id);

				// Add direct addresses if available
				for addr_str in &persisted_device.last_seen_addresses {
					if let Ok(addr) = addr_str.parse() {
						node_addr = node_addr.with_direct_addresses([addr]);
					}
				}

				// Attempt connection with Iroh
				match endpoint.connect(node_addr, &[]).await {
					Ok(_conn) => {
						logger
							.info(&format!("✅ Successfully connected to device {}", device_id))
							.await;

						// Send connection established command
						let _ = sender
							.send(EventLoopCommand::ConnectionEstablished { device_id, node_id });
					}
					Err(e) => {
						logger
							.error(&format!("❌ Failed to connect to device {}: {}", device_id, e))
							.await;
					}
				}
			}
		}
	}

	/// Start periodic reconnection attempts for disconnected devices
	async fn start_periodic_reconnection(&self) {
		let device_registry = self.device_registry.clone();
		let command_sender = self.command_sender.clone();
		let endpoint = self.endpoint.clone();
		let logger = self.logger.clone();

		tokio::spawn(async move {
			let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));

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
							logger
								.info(&format!(
									"🔄 Attempting periodic reconnection to device: {}",
									device_id
								))
								.await;
							let cmd_sender = command_sender.clone();
							let ep = endpoint.clone();
							let logger_clone = logger.clone();
							tokio::spawn(async move {
								Self::attempt_device_reconnection(
									device_id,
									persisted_device,
									cmd_sender,
									ep,
									logger_clone,
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

	/// Get our node ID
	pub fn node_id(&self) -> NodeId {
		self.node_id
	}

	/// Get connected devices
	pub async fn get_connected_devices(&self) -> Vec<DeviceInfo> {
		self.device_registry.read().await.get_connected_devices()
	}

	/// Get raw connected nodes directly from endpoint
	pub async fn get_raw_connected_nodes(&self) -> Vec<NodeId> {
		let connections = self.active_connections.read().await;
		connections.keys().cloned().collect()
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

	/// Publish a discovery record for pairing session
	pub async fn publish_discovery_record(&self, key: &[u8], value: Vec<u8>) -> Result<()> {
		// For pairing, we don't need to explicitly publish with LocalSwarmDiscovery
		// It automatically advertises our node on the local network
		// The pairing protocol will handle session-specific discovery via direct connection
		Ok(())
	}

	/// Query a discovery record for pairing session
	pub async fn query_discovery_record(&self, key: &[u8]) -> Result<Vec<NodeAddr>> {
		// With LocalSwarmDiscovery, we can't query specific records
		// Instead, we'll discover all local nodes and filter by pairing session
		// For now, return empty - pairing will use direct connection after discovery
		Ok(Vec::new())
	}

	/// Get currently connected nodes for direct pairing attempts
	pub async fn get_connected_nodes(&self) -> Vec<NodeId> {
		// Get connected nodes from device registry
		let registry = self.device_registry.read().await;
		registry.get_connected_nodes()
	}

	/// Get the local device ID
	pub fn device_id(&self) -> Uuid {
		self.identity.device_id()
	}

	/// Get the command sender for the event loop
	pub fn command_sender(&self) -> Option<&mpsc::UnboundedSender<event_loop::EventLoopCommand>> {
		self.command_sender.as_ref()
	}

	/// Send message to a specific node (bypassing device lookup)
	pub async fn send_message_to_node(
		&self,
		node_id: NodeId,
		protocol: &str,
		data: Vec<u8>,
	) -> Result<()> {
		if let Some(command_sender) = &self.command_sender {
			let command = event_loop::EventLoopCommand::SendMessageToNode {
				node_id,
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

	/// Connect to a node at a specific address
	pub async fn connect_to_node(&self, node_addr: NodeAddr) -> Result<()> {
		if let Some(endpoint) = &self.endpoint {
			// Use pairing ALPN for initial connection during pairing
			let conn = endpoint
				.connect(node_addr.clone(), PAIRING_ALPN)
				.await
				.map_err(|e| {
					NetworkingError::ConnectionFailed(format!("Failed to connect: {}", e))
				})?;

			// Track the outbound connection
			let node_id = node_addr.node_id;
			{
				let mut connections = self.active_connections.write().await;
				connections.insert(node_id, conn);
				self.logger
					.info(&format!("🔗 Bob: Tracked outbound connection to {}", node_id))
					.await;
			}

			Ok(())
		} else {
			Err(NetworkingError::ConnectionFailed(
				"Networking not started".to_string(),
			))
		}
	}

	/// Get our node address for advertising
	pub async fn get_node_addr(&self) -> Result<NodeAddr> {
		if let Some(endpoint) = &self.endpoint {
			endpoint
				.node_addr()
				.await
				.map_err(|e| NetworkingError::Protocol(format!("Failed to get node addr: {}", e)))
		} else {
			Err(NetworkingError::ConnectionFailed(
				"Networking not started".to_string(),
			))
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

		// Generate session ID
		let session_id = uuid::Uuid::new_v4();
		let pairing_code =
			crate::services::networking::protocols::pairing::PairingCode::from_session_id(
				session_id,
			);

		// Start pairing session
		pairing_handler
			.start_pairing_session_with_id(session_id, pairing_code.clone())
			.await?;

		// Register in device registry
		let initiator_device_id = self.device_id();
		let initiator_node_id = self.node_id();
		let device_registry = self.device_registry();
		{
			let mut registry = device_registry.write().await;
			registry.start_pairing(initiator_device_id, initiator_node_id, session_id)?;
		}

		// Get our node address for advertising
		let node_addr = self.get_node_addr().await?;

		self.logger
			.info(&format!("🌐 Initiator: Node address: {:?}", node_addr))
			.await;
		self.logger
			.info(&format!(
				"🌐 Initiator: Direct addresses: {:?}",
				node_addr.direct_addresses().collect::<Vec<_>>()
			))
			.await;
		self.logger
			.info(&format!("🌐 Initiator: Relay URL: {:?}", node_addr.relay_url()))
			.await;

		// Create pairing advertisement
		let node_addr_info = crate::services::networking::protocols::pairing::types::NodeAddrInfo {
			node_id: self.node_id().to_string(),
			direct_addresses: node_addr
				.direct_addresses()
				.map(|addr| addr.to_string())
				.collect(),
			relay_url: node_addr.relay_url().map(|u| u.to_string()),
		};

		let advertisement = crate::services::networking::protocols::pairing::PairingAdvertisement {
			node_id: self.node_id().to_string(),
			node_addr_info,
			device_info: pairing_handler.get_device_info().await?,
			expires_at: chrono::Utc::now() + chrono::Duration::minutes(5),
			created_at: chrono::Utc::now(),
		};

		// Publish to discovery
		let key = session_id.as_bytes();
		let value = serde_json::to_vec(&advertisement)
			.map_err(|e| NetworkingError::Protocol(e.to_string()))?;

		self.publish_discovery_record(key, value.clone()).await?;

		// For local testing: also write the advertisement to a file
		// This simulates what a DHT would do for peer discovery
		if let Ok(temp_dir) = std::env::var("SPACEDRIVE_TEST_DIR") {
			let session_file = format!("{}/pairing_session_{}.json", temp_dir, session_id);
			if let Err(e) = std::fs::write(&session_file, &value) {
				self.logger
					.warn(&format!("Warning: Could not write pairing session file: {}", e))
					.await;
			} else {
				self.logger
					.info(&format!(
						"📄 Initiator: Wrote pairing session info to {}",
						session_file
					))
					.await;
			}
		}

		let expires_in = 300; // 5 minutes

		Ok((pairing_code.to_string(), expires_in))
	}

	/// Start pairing as a joiner (connects using pairing code)
	pub async fn start_pairing_as_joiner(&self, code: &str) -> Result<()> {
		// Parse BIP39 pairing code
		let pairing_code =
			crate::services::networking::protocols::pairing::PairingCode::from_string(code)?;
		let session_id = pairing_code.session_id();

		// Get pairing handler
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

		// Join pairing session
		pairing_handler
			.join_pairing_session(session_id, pairing_code)
			.await?;

		// With LocalSwarmDiscovery, peers should auto-discover on the local network
		// Wait a moment for discovery to happen
		self.logger
			.info("🔍 Waiting for peer discovery on local network...")
			.await;
		tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

		// Query discovery for Alice's advertisement (even though it returns empty with LocalSwarmDiscovery)
		let key = session_id.as_bytes();
		let mut node_addrs = self.query_discovery_record(key).await?;

		// For local testing: also check for file-based session advertisement
		if let Ok(temp_dir) = std::env::var("SPACEDRIVE_TEST_DIR") {
			let session_file = format!("{}/pairing_session_{}.json", temp_dir, session_id);
			if let Ok(data) = std::fs::read(&session_file) {
				if let Ok(advertisement) = serde_json::from_slice::<
					crate::services::networking::protocols::pairing::PairingAdvertisement,
				>(&data)
				{
					if let Ok(initiator_node_addr) = advertisement.node_addr() {
						println!(
							"📄 Bob: Found Initiator's session info, attempting connection..."
						);
						node_addrs.push(initiator_node_addr);
					}
				}
			}
		}

		// Try to connect to discovered nodes
		for node_addr in node_addrs {
			self.logger
				.info("🔗 Joiner: Attempting to connect to node...")
				.await;
			self.logger
				.info(&format!("🔗 Joiner: Node address: {:?}", node_addr))
				.await;
			self.logger
				.info(&format!(
					"🔗 Joiner: Direct addresses: {:?}",
					node_addr.direct_addresses().collect::<Vec<_>>()
				))
				.await;
			self.logger
				.info(&format!("🔗 Joiner: Relay URL: {:?}", node_addr.relay_url()))
				.await;

			if let Err(e) = self.connect_to_node(node_addr.clone()).await {
				self.logger
					.error(&format!("❌ Joiner: Failed to connect to {:?}: {}", node_addr, e))
					.await;
			} else {
				self.logger
					.info("✅ Joiner: Successfully connected to peer!")
					.await;
			}
		}

		// Wait a moment for connections to be properly tracked
		tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

		// Send pairing request to any connected nodes
		let connected_nodes = self.get_raw_connected_nodes().await;
		println!(
			"🔍 Joiner: Found {} raw connected nodes",
			connected_nodes.len()
		);
		if !connected_nodes.is_empty() {
			println!(
				"📡 Joiner: Found {} connected nodes, sending pairing requests...",
				connected_nodes.len()
			);
			for node_id in connected_nodes {
				// Get local device info
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
					crate::services::networking::protocols::pairing::messages::PairingMessage::PairingRequest {
						session_id,
						device_info: local_device_info,
						public_key: self.identity().public_key_bytes(),
					};

				// Send via Iroh stream using the pairing handler and wait for response
				if let Some(endpoint) = &self.endpoint {
					let registry = self.protocol_registry();
					let guard = registry.read().await;
					if let Some(handler) = guard.get_handler("pairing") {
						if let Some(pairing_handler) =
							handler.as_any().downcast_ref::<PairingProtocolHandler>()
						{
							println!("📤 Bob: Sending pairing request to node {}", node_id);
							match pairing_handler
								.send_pairing_message_to_node(endpoint, node_id, &pairing_request)
								.await
							{
								Ok(Some(response)) => {
									println!("📥 Joiner: Received response from Initiator!");
									// Process the response via the trait's handle_response method
									if let Ok(msg_bytes) = serde_json::to_vec(&response) {
										let device_id = self.device_id(); // Bob's own device ID
										let _ = handler
											.handle_response(device_id, node_id, msg_bytes)
											.await;
									}
									// Stop sending more requests since we got a response
									break;
								}
								Ok(None) => {
									println!("⚠️ Joiner: No response received from Initiator");
								}
								Err(e) => {
									println!("❌ Joiner: Failed to send pairing request: {}", e);
								}
							}
						}
					}
				}
			}
		}

		// Ensure pairing requests are sent with polling
		self.ensure_pairing_requests_sent(session_id).await?;

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
	async fn ensure_pairing_requests_sent(&self, session_id: uuid::Uuid) -> Result<()> {
		const MAX_WAIT_TIME: u64 = 15000; // 15 seconds
		const POLL_INTERVAL: u64 = 500; // Check every 500ms
		let start_time = std::time::Instant::now();

		loop {
			// First, check if the session has already advanced
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

			// Check for connected nodes and send the request
			let connected_nodes = self.get_raw_connected_nodes().await;
			if !connected_nodes.is_empty() {
				for node_id in &connected_nodes {
					let local_device_info = {
						let device_registry = self.device_registry();
						let registry = device_registry.read().await;
						registry.get_local_device_info().unwrap_or_else(|_| {
							crate::services::networking::device::DeviceInfo {
								device_id: self.device_id(),
								device_name: "Joiner's Test Device".to_string(),
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
						crate::services::networking::protocols::pairing::messages::PairingMessage::PairingRequest {
							session_id,
							device_info: local_device_info,
							public_key: self.identity().public_key_bytes(),
						};

					// Send via Iroh stream using the pairing handler and wait for response
					if let Some(endpoint) = &self.endpoint {
						let registry = self.protocol_registry();
						let guard = registry.read().await;
						if let Some(handler) = guard.get_handler("pairing") {
							if let Some(pairing_handler) =
								handler.as_any().downcast_ref::<PairingProtocolHandler>()
							{
								match pairing_handler
									.send_pairing_message_to_node(
										endpoint,
										*node_id,
										&pairing_request,
									)
									.await
								{
									Ok(Some(response)) => {
										println!("📥 Joiner: Received challenge response from Initiator!");
										// Process the response via the trait's handle_response method
										if let Ok(msg_bytes) = serde_json::to_vec(&response) {
											let device_id = self.device_id(); // Bob's own device ID
											let _ = handler
												.handle_response(device_id, *node_id, msg_bytes)
												.await;
										}
										// Return early since we got a response
										return Ok(());
									}
									Ok(None) => {
										println!("⚠️ Joiner: No response received in ensure_pairing_requests_sent");
									}
									Err(e) => {
										println!("❌ Joiner: Failed to send pairing request in ensure_pairing_requests_sent: {}", e);
									}
								}
							}
						}
					}
				}
			}

			// Check for timeout
			if start_time.elapsed().as_millis() > MAX_WAIT_TIME as u128 {
				return Err(NetworkingError::Protocol(
					"Pairing timeout: Did not receive challenge from Initiator.".to_string(),
				));
			}

			tokio::time::sleep(tokio::time::Duration::from_millis(POLL_INTERVAL)).await;
		}
	}
}

// Ensure NetworkingService is Send + Sync for proper async usage
unsafe impl Send for NetworkingService {}
unsafe impl Sync for NetworkingService {}
