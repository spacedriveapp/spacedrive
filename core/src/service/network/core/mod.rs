//! Core networking engine with Iroh P2P

pub mod event_loop;

use crate::device::DeviceManager;
use crate::service::network::{
	device::{DeviceInfo, DeviceRegistry},
	protocol::{pairing::PairingProtocolHandler, ProtocolRegistry},
	utils::{logging::NetworkLogger, NetworkIdentity},
	NetworkingError, Result,
};
use iroh::discovery::{mdns::MdnsDiscovery, Discovery};
use iroh::endpoint::Connection;
use iroh::{Endpoint, NodeAddr, NodeId, RelayMode, Watcher};
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
		library_key_manager: Arc<crate::crypto::library_key_manager::LibraryKeyManager>,
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
		let device_registry = Arc::new(RwLock::new(DeviceRegistry::new(
			device_manager,
			data_dir,
			logger.clone(),
		)?));

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

		// Create discovery service - using mDNS discovery
		let discovery = MdnsDiscovery::builder();

		self.logger
			.info(&format!("Created MdnsDiscovery for node {}", self.node_id))
			.await;

		// Create endpoint with discovery
		let endpoint = Endpoint::builder()
			.secret_key(secret_key)
			.alpns(vec![
				PAIRING_ALPN.to_vec(),
				FILE_TRANSFER_ALPN.to_vec(),
				MESSAGING_ALPN.to_vec(),
			])
			.relay_mode(iroh::RelayMode::Default)
			.add_discovery(discovery)
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
			self.logger.clone(),
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
				"Loaded {} paired devices from persistence",
				loaded_device_ids.len()
			))
			.await;

		// Get devices that should auto-reconnect
		let auto_reconnect_devices = device_registry.get_auto_reconnect_devices().await?;
		self.logger
			.info(&format!(
				"Found {} devices for auto-reconnection",
				auto_reconnect_devices.len()
			))
			.await;

		drop(device_registry); // Release the lock for async operations

		// Give discovery service time to start up before attempting reconnections
		tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

		// Start background reconnection attempts
		self.start_background_reconnection(auto_reconnect_devices)
			.await;

		Ok(())
	}

	/// Start background reconnection attempts for paired devices
	async fn start_background_reconnection(
		&self,
		auto_reconnect_devices: Vec<(Uuid, crate::service::network::device::PersistedPairedDevice)>,
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
		persisted_device: crate::service::network::device::PersistedPairedDevice,
		command_sender: Option<tokio::sync::mpsc::UnboundedSender<EventLoopCommand>>,
		endpoint: Option<Endpoint>,
		logger: Arc<dyn NetworkLogger>,
	) {
		logger
			.info(&format!(
				"Starting reconnection attempts for device: {}",
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

				// If no direct addresses, let discovery find the node
				if node_addr.direct_addresses().count() == 0 {
					logger
						.info(&format!(
							"No direct addresses for device {}, relying on discovery",
							device_id
						))
						.await;
				}

				// Attempt connection with retries to give discovery time to work
				let mut retry_count = 0;
				let max_retries = 10;
				let retry_delay = tokio::time::Duration::from_secs(5);

				loop {
					// Use MESSAGING_ALPN for reconnection to paired devices
					match endpoint.connect(node_addr.clone(), MESSAGING_ALPN).await {
						Ok(_conn) => {
							logger
								.info(&format!("Successfully connected to device {}", device_id))
								.await;

							// Send connection established command
							let _ = sender.send(EventLoopCommand::ConnectionEstablished {
								device_id,
								node_id,
							});
							break;
						}
						Err(e) => {
							retry_count += 1;
							if retry_count >= max_retries {
								logger
									.error(&format!(
										"Failed to connect to device {} after {} attempts: {}",
										device_id, max_retries, e
									))
									.await;
								break;
							} else {
								logger
									.info(&format!(
										"Connection attempt {} of {} failed for device {}, retrying in {:?}...",
										retry_count, max_retries, device_id, retry_delay
									))
									.await;
								tokio::time::sleep(retry_delay).await;
							}
						}
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
						let is_disconnected =
							{
								let registry = device_registry.read().await;
								if let Some(device_state) = registry.get_device_state(device_id) {
									matches!(device_state, crate::service::network::device::DeviceState::Disconnected { .. })
								} else {
									true // Not in registry, try to reconnect
								}
							};

						if is_disconnected {
							logger
								.info(&format!(
									"Attempting periodic reconnection to device: {}",
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
					.info(&format!("Tracked outbound connection to {}", node_id))
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
	pub fn get_node_addr(&self) -> Result<Option<NodeAddr>> {
		if let Some(endpoint) = &self.endpoint {
			Ok(endpoint.node_addr().get())
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
			.downcast_ref::<crate::service::network::protocol::PairingProtocolHandler>()
			.ok_or(NetworkingError::Protocol(
				"Invalid pairing handler type".to_string(),
			))?;

		// Generate session ID
		let session_id = uuid::Uuid::new_v4();
		let pairing_code =
			crate::service::network::protocol::pairing::PairingCode::from_session_id(session_id);

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
		let mut node_addr = self.get_node_addr()?;

		// If we don't have any direct addresses yet, wait a bit for them to be discovered
		if let Some(addr) = &node_addr {
			if addr.direct_addresses().count() == 0 {
				self.logger
					.info("No direct addresses discovered yet, waiting for endpoint to discover addresses...")
					.await;

				// Wait up to 5 seconds for addresses to be discovered
				let mut attempts = 0;
				const MAX_ATTEMPTS: u32 = 10;
				const WAIT_TIME_MS: u64 = 500;

				while attempts < MAX_ATTEMPTS {
					tokio::time::sleep(tokio::time::Duration::from_millis(WAIT_TIME_MS)).await;
					node_addr = self.get_node_addr()?;

					if let Some(addr) = &node_addr {
						if addr.direct_addresses().count() > 0 {
							self.logger
								.info(&format!(
									"Discovered {} direct addresses",
									addr.direct_addresses().count()
								))
								.await;
							break;
						}
					}

					attempts += 1;
				}
			}
		}

		if node_addr
			.as_ref()
			.map_or(true, |addr| addr.direct_addresses().count() == 0)
		{
			self.logger
				.warn("No direct addresses discovered after waiting, proceeding with relay-only address")
				.await;
		}

		self.logger
			.info(&format!("Node address: {:?}", node_addr))
			.await;
		self.logger
			.info(&format!(
				"Direct addresses: {:?}",
				node_addr
					.as_ref()
					.map(|addr| addr.direct_addresses().collect::<Vec<_>>())
					.unwrap_or_default()
			))
			.await;
		self.logger
			.info(&format!(
				"Relay URL: {:?}",
				node_addr.as_ref().and_then(|addr| addr.relay_url())
			))
			.await;

		// Create pairing advertisement
		let node_addr_info = crate::service::network::protocol::pairing::types::NodeAddrInfo {
			node_id: self.node_id().to_string(),
			direct_addresses: node_addr
				.as_ref()
				.map(|addr| {
					addr.direct_addresses()
						.map(|a| a.to_string())
						.collect::<Vec<_>>()
				})
				.unwrap_or_default(),
			relay_url: node_addr
				.as_ref()
				.and_then(|addr| addr.relay_url())
				.map(|u| u.to_string()),
		};

		let advertisement = crate::service::network::protocol::pairing::PairingAdvertisement {
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
		let temp_dir = std::env::var("SPACEDRIVE_TEST_DIR")
			.unwrap_or_else(|_| "/tmp/spacedrive-pairing".to_string());

		// Create directory if it doesn't exist
		if let Err(e) = std::fs::create_dir_all(&temp_dir) {
			self.logger
				.warn(&format!(
					"Warning: Could not create pairing directory {}: {}",
					temp_dir, e
				))
				.await;
		} else {
			let session_file = format!("{}/pairing_session_{}.json", temp_dir, session_id);
			if let Err(e) = std::fs::write(&session_file, &value) {
				self.logger
					.warn(&format!(
						"Warning: Could not write pairing session file: {}",
						e
					))
					.await;
			} else {
				self.logger
					.info(&format!("Wrote pairing session info to {}", session_file))
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
			crate::service::network::protocol::pairing::PairingCode::from_string(code)?;
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
			.downcast_ref::<crate::service::network::protocol::PairingProtocolHandler>()
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
			.info("Waiting for peer discovery on local network...")
			.await;
		tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

		// Query discovery for initiator's advertisement (even though it returns empty with LocalSwarmDiscovery)
		let key = session_id.as_bytes();
		let mut node_addrs = self.query_discovery_record(key).await?;

		// Check for file-based session advertisement
		// This is a temporary solution until we have proper DHT or mDNS discovery
		let temp_dir = std::env::var("SPACEDRIVE_TEST_DIR")
			.unwrap_or_else(|_| "/tmp/spacedrive-pairing".to_string());

		let session_file = format!("{}/pairing_session_{}.json", temp_dir, session_id);
		if let Ok(data) = std::fs::read(&session_file) {
			if let Ok(advertisement) = serde_json::from_slice::<
				crate::service::network::protocol::pairing::PairingAdvertisement,
			>(&data)
			{
				if let Ok(initiator_node_addr) = advertisement.node_addr() {
					self.logger
						.info("Found Initiator's session info, attempting connection...")
						.await;
					node_addrs.push(initiator_node_addr);
				}
			}
		} else {
			self.logger
				.debug(&format!(
					"No pairing session file found at {}",
					session_file
				))
				.await;
		}

		// Try to connect to discovered nodes first
		for node_addr in node_addrs {
			self.logger.info("Attempting to connect to node...").await;
			self.logger
				.info(&format!("Node address: {:?}", node_addr))
				.await;
			self.logger
				.info(&format!(
					"Direct addresses: {:?}",
					node_addr.direct_addresses().collect::<Vec<_>>()
				))
				.await;
			self.logger
				.info(&format!("Relay URL: {:?}", node_addr.relay_url()))
				.await;

			if let Err(e) = self.connect_to_node(node_addr.clone()).await {
				self.logger
					.error(&format!("Failed to connect to {:?}: {}", node_addr, e))
					.await;
			} else {
				self.logger.info("Successfully connected to peer!").await;
			}
		}

		// Wait a moment for connections to be properly tracked
		tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

		// Send pairing request to any connected nodes
		let connected_nodes = self.get_raw_connected_nodes().await;
		self.logger
			.debug(&format!(
				"Found {} raw connected nodes",
				connected_nodes.len()
			))
			.await;

		// If no nodes are connected yet, try to discover all peers on the network
		// and attempt to connect to each one - the initiator will respond to our pairing request
		if connected_nodes.is_empty() {
			self.logger
				.info("No connected nodes found, attempting to discover all peers on local network...")
				.await;

			// Get all discovered peers through the endpoint's discovery service
			if let Some(endpoint) = &self.endpoint {
				// LocalSwarmDiscovery should have discovered peers by now
				// We need to try connecting to all discovered nodes since we don't know which one is the initiator

				// Get our own node address to broadcast it
				let our_node_addr = endpoint.node_addr().get();

				self.logger
					.info(&format!(
						"Our node address for pairing: {:?}",
						our_node_addr
					))
					.await;

				// Since we can't directly query discovered nodes from LocalSwarmDiscovery,
				// we'll implement a broadcast approach where we try to connect to any node
				// that might be listening with the pairing ALPN

				// For now, let's wait a bit longer for discovery and connection attempts
				self.logger
					.info("Waiting additional time for local network discovery...")
					.await;
				tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

				// Check again for connected nodes
				let connected_nodes = self.get_raw_connected_nodes().await;
				if connected_nodes.is_empty() {
					self.logger
						.warn("Still no connected nodes after extended discovery period")
						.await;
					self.logger
						.info("Ensure both devices are on the same local network and the initiator is running")
						.await;

					// As a last resort, if we're in a test environment with the pairing session file,
					// we already tried to connect to it above
					return Err(NetworkingError::Protocol(
						"Failed to discover initiator on local network. Ensure both devices are on the same network.".to_string()
					));
				}
			}
		}

		// Get the potentially updated list of connected nodes
		let connected_nodes = self.get_raw_connected_nodes().await;

		if !connected_nodes.is_empty() {
			self.logger
				.info(&format!(
					"Found {} connected nodes, sending pairing requests...",
					connected_nodes.len()
				))
				.await;
			for node_id in connected_nodes {
				// Get local device info
				let local_device_info = {
					let device_registry = self.device_registry();
					let registry = device_registry.read().await;
					registry.get_local_device_info().unwrap_or_else(|_| {
						crate::service::network::device::DeviceInfo {
							device_id: self.device_id(),
							device_name: "Joiner Device".to_string(),
							device_type: crate::service::network::device::DeviceType::Desktop,
							os_version: std::env::consts::OS.to_string(),
							app_version: env!("CARGO_PKG_VERSION").to_string(),
							network_fingerprint: self.identity().network_fingerprint(),
							last_seen: chrono::Utc::now(),
						}
					})
				};

				let pairing_request =
					crate::service::network::protocol::pairing::messages::PairingMessage::PairingRequest {
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
							self.logger
								.info(&format!("Sending pairing request to node {}", node_id))
								.await;
							match pairing_handler
								.send_pairing_message_to_node(endpoint, node_id, &pairing_request)
								.await
							{
								Ok(Some(response)) => {
									self.logger.info("Received response from Initiator!").await;
									// Process the response via the trait's handle_response method
									if let Ok(msg_bytes) = serde_json::to_vec(&response) {
										let device_id = self.device_id(); // Joiner's own device ID
										let _ = handler
											.handle_response(device_id, node_id, msg_bytes)
											.await;
									}
									// Stop sending more requests since we got a response
									break;
								}
								Ok(None) => {
									self.logger
										.warn("No response received from Initiator")
										.await;
								}
								Err(e) => {
									self.logger
										.error(&format!("Failed to send pairing request: {}", e))
										.await;
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
	pub async fn get_pairing_status(&self) -> Result<Vec<crate::service::network::PairingSession>> {
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
		if let Some(pairing_handler) = pairing_handler
			.as_any()
			.downcast_ref::<crate::service::network::protocol::PairingProtocolHandler>(
		) {
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
				if let Some(handler) = pairing_handler
					.as_any()
					.downcast_ref::<crate::service::network::protocol::PairingProtocolHandler>(
				) {
					let sessions = handler.get_active_sessions().await;
					if let Some(session) = sessions.iter().find(|s| s.id == session_id) {
						if !matches!(
							session.state,
							crate::service::network::protocol::pairing::PairingState::Scanning
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
							crate::service::network::device::DeviceInfo {
								device_id: self.device_id(),
								device_name: "Joiner's Test Device".to_string(),
								device_type: crate::service::network::device::DeviceType::Desktop,
								os_version: std::env::consts::OS.to_string(),
								app_version: env!("CARGO_PKG_VERSION").to_string(),
								network_fingerprint: self.identity().network_fingerprint(),
								last_seen: chrono::Utc::now(),
							}
						})
					};

					let pairing_request =
						crate::service::network::protocol::pairing::messages::PairingMessage::PairingRequest {
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
										self.logger
											.info("Received challenge response from Initiator!")
											.await;
										// Process the response via the trait's handle_response method
										if let Ok(msg_bytes) = serde_json::to_vec(&response) {
											let device_id = self.device_id(); // Joiner's own device ID
											let _ = handler
												.handle_response(device_id, *node_id, msg_bytes)
												.await;
										}
										// Return early since we got a response
										return Ok(());
									}
									Ok(None) => {
										self.logger
											.warn("No response received in ensure_pairing_requests_sent")
											.await;
									}
									Err(e) => {
										self.logger
											.error(&format!("Failed to send pairing request in ensure_pairing_requests_sent: {}", e))
											.await;
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
