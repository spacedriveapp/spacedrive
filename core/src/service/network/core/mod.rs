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
use iroh::{Endpoint, NodeAddr, NodeId, RelayMode, RelayUrl, Watcher};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

pub use event_loop::{EventLoopCommand, NetworkingEventLoop};

/// Protocol ALPN identifiers
pub const PAIRING_ALPN: &[u8] = b"spacedrive/pairing/1";
pub const FILE_TRANSFER_ALPN: &[u8] = b"spacedrive/filetransfer/1";
pub const MESSAGING_ALPN: &[u8] = b"spacedrive/messaging/1";
pub const SYNC_ALPN: &[u8] = b"spacedrive/sync/1";

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
			.info(&format!(
				"Created MdnsDiscovery builder for node {}",
				self.node_id
			))
			.await;

		// Create endpoint with discovery
		let endpoint = Endpoint::builder()
			.secret_key(secret_key)
			.alpns(vec![
				PAIRING_ALPN.to_vec(),
				FILE_TRANSFER_ALPN.to_vec(),
				MESSAGING_ALPN.to_vec(),
				SYNC_ALPN.to_vec(),
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

		self.logger
			.info("Endpoint bound successfully with mDNS discovery enabled")
			.await;

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

		// Start periodic health checks for connected devices
		// TODO: Health checks opening streams causes connection closure
		// Need to implement proper QUIC keep-alive instead
		// self.start_health_check_task().await;

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
		// Deterministic reconnection: only the device with the lower NodeId initiates
		// This prevents both sides from simultaneously trying to connect
		let endpoint_ref = match &endpoint {
			Some(ep) => ep,
			None => {
				logger.warn("No endpoint available for reconnection").await;
				return;
			}
		};

		let my_node_id = endpoint_ref.node_id();
		let remote_node_id = match persisted_device
			.device_info
			.network_fingerprint
			.node_id
			.parse::<NodeId>()
		{
			Ok(id) => id,
			Err(e) => {
				logger
					.warn(&format!("Failed to parse remote node ID: {}", e))
					.await;
				return;
			}
		};

		// Deterministic rule: only device with lower NodeId initiates outbound connections
		// This prevents both sides from creating competing connections
		if my_node_id > remote_node_id {
			logger
				.debug(&format!(
					"Skipping outbound reconnection to {} - waiting for them to connect to us (NodeId rule: {} > {})",
					persisted_device.device_info.device_name,
					my_node_id,
					remote_node_id
				))
				.await;
			return;
		}

		logger
			.info(&format!(
				"NodeId rule: {} < {} - we should initiate connection",
				my_node_id, remote_node_id
			))
			.await;

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
						Ok(conn) => {
							logger
								.info(&format!("Successfully connected to device {}", device_id))
								.await;

							// Track this outbound connection so it persists
							let _ = sender.send(EventLoopCommand::TrackOutboundConnection {
								node_id,
								conn: conn.clone(),
							});

							// Connection established - don't open streams immediately
							// Let connections be idle until actually needed for data transfer
							// This prevents connection closure after initial ping/pong
							logger
								.info(&format!("Connection established to device {}", device_id))
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

	/// Start periodic health checks for connected devices
	async fn start_health_check_task(&self) {
		let device_registry = self.device_registry.clone();
		let command_sender = self.command_sender.clone();
		let endpoint = self.endpoint.clone();
		let logger = self.logger.clone();
		let active_connections = self.active_connections.clone();

		tokio::spawn(async move {
			let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
			let mut failed_pings: std::collections::HashMap<uuid::Uuid, u32> =
				std::collections::HashMap::new();

			loop {
				interval.tick().await;

				// Get all connected devices
				let connected_devices: Vec<(uuid::Uuid, iroh::NodeId)> = {
					let registry = device_registry.read().await;
					registry
						.get_all_devices()
						.into_iter()
						.filter_map(|(device_id, state)| {
							if let crate::service::network::device::DeviceState::Connected {
								info,
								..
							} = state
							{
								if let Ok(node_id) =
									info.network_fingerprint.node_id.parse::<iroh::NodeId>()
								{
									Some((device_id, node_id))
								} else {
									None
								}
							} else {
								None
							}
						})
						.collect()
				};

				if !connected_devices.is_empty() {
					logger
						.debug(&format!(
							"Health check: pinging {} connected devices",
							connected_devices.len()
						))
						.await;
				}

				for (device_id, node_id) in connected_devices {
					// Check if connection still exists
					let has_connection = {
						let connections = active_connections.read().await;
						connections.contains_key(&node_id)
					};

					if !has_connection {
						// Connection was lost but device is still marked as connected
						logger
							.warn(&format!(
								"Device {} marked as connected but no active connection found",
								device_id
							))
							.await;

						if let Some(sender) = &command_sender {
							let _ = sender.send(crate::service::network::core::event_loop::EventLoopCommand::ConnectionLost {
								device_id,
								node_id,
								reason: "Connection not found in active connections".to_string(),
							});
						}
						failed_pings.remove(&device_id);
						continue;
					}

					// Send ping message using existing connection
					let ping_msg = crate::service::network::protocol::messaging::Message::Ping {
						timestamp: chrono::Utc::now(),
						payload: None,
					};

					if let Ok(ping_data) = serde_json::to_vec(&ping_msg) {
						// Use existing connection from active_connections
						let connections = active_connections.read().await;
						let ping_result = if let Some(conn) = connections.get(&node_id) {
							tokio::time::timeout(tokio::time::Duration::from_secs(10), async {
								match conn.open_bi().await {
									Ok((mut send, mut recv)) => {
										use tokio::io::{AsyncReadExt, AsyncWriteExt};

										// Send ping with length prefix
										let len = ping_data.len() as u32;
										if send.write_all(&len.to_be_bytes()).await.is_err() {
											return false;
										}
										if send.write_all(&ping_data).await.is_err() {
											return false;
										}
										if send.flush().await.is_err() {
											return false;
										}

										// Wait for pong response
										let mut len_buf = [0u8; 4];
										if recv.read_exact(&mut len_buf).await.is_err() {
											return false;
										}
										let resp_len = u32::from_be_bytes(len_buf) as usize;

										let mut resp_buf = vec![0u8; resp_len];
										if recv.read_exact(&mut resp_buf).await.is_err() {
											return false;
										}

										// Verify it's a pong
										if let Ok(msg) = serde_json::from_slice::<
											crate::service::network::protocol::messaging::Message,
										>(&resp_buf)
										{
											matches!(msg, crate::service::network::protocol::messaging::Message::Pong { .. })
										} else {
											false
										}
									}
									Err(_) => false,
								}
							})
							.await
						} else {
							// No active connection found
							logger
								.warn(&format!(
									"No active connection for health check to device {}",
									device_id
								))
								.await;
							Ok(false)
						};
						drop(connections);

						match ping_result {
							Ok(true) => {
								// Ping successful, reset failure count
								failed_pings.remove(&device_id);
								logger
									.debug(&format!(
										"Health check: device {} responded to ping",
										device_id
									))
									.await;
							}
							Ok(false) | Err(_) => {
								// Ping failed or timed out
								let fail_count = failed_pings.entry(device_id).or_insert(0);
								*fail_count += 1;

								logger
									.warn(&format!(
										"Health check: device {} failed ping (attempt {}/3)",
										device_id, fail_count
									))
									.await;

								if *fail_count >= 3 {
									// Device has failed 3 consecutive pings, mark as disconnected
									logger
										.error(&format!(
												"Health check: device {} failed 3 consecutive pings, marking as disconnected",
												device_id
											))
										.await;

									if let Some(sender) = &command_sender {
										let _ = sender.send(crate::service::network::core::event_loop::EventLoopCommand::ConnectionLost {
												device_id,
												node_id,
												reason: "Failed health check (3 consecutive ping timeouts)".to_string(),
											});
									}
									failed_pings.remove(&device_id);
								}
							}
						}
					}
				}
			}
		});
	}

	/// Stop the networking service
	pub async fn shutdown(&self) -> Result<()> {
		// Send goodbye messages to all connected devices
		self.logger
			.info("Sending disconnect notifications to connected devices")
			.await;

		let connected_devices: Vec<(uuid::Uuid, iroh::NodeId)> = {
			let registry = self.device_registry.read().await;
			registry
				.get_all_devices()
				.into_iter()
				.filter_map(|(device_id, state)| {
					if let crate::service::network::device::DeviceState::Connected {
						info, ..
					} = state
					{
						if let Ok(node_id) =
							info.network_fingerprint.node_id.parse::<iroh::NodeId>()
						{
							Some((device_id, node_id))
						} else {
							None
						}
					} else {
						None
					}
				})
				.collect()
		};

		// Send goodbye message to each connected device
		let device_count = connected_devices.len();
		for (device_id, node_id) in connected_devices {
			let goodbye_msg = crate::service::network::protocol::messaging::Message::Goodbye {
				reason: "Daemon shutting down".to_string(),
				timestamp: chrono::Utc::now(),
			};

			if let Ok(goodbye_data) = serde_json::to_vec(&goodbye_msg) {
				if let Some(command_sender) = &self.command_sender {
					// Best effort - don't block if it fails
					let _ = command_sender.send(EventLoopCommand::SendMessageToNode {
						node_id,
						protocol: "messaging".to_string(),
						data: goodbye_data,
					});
				}
			}

			self.logger
				.debug(&format!(
					"Sent disconnect notification to device {}",
					device_id
				))
				.await;
		}

		// Give messages time to be sent
		if device_count > 0 {
			tokio::time::sleep(std::time::Duration::from_millis(500)).await;
		}

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

	/// Send a request to a device and wait for response
	pub async fn send_library_request(
		&self,
		device_id: Uuid,
		request: crate::service::network::protocol::library_messages::LibraryMessage,
	) -> Result<crate::service::network::protocol::library_messages::LibraryMessage> {
		// Get node_id from device registry
		let registry = self.device_registry.read().await;
		let node_id = registry
			.get_node_by_device(device_id)
			.ok_or_else(|| NetworkingError::DeviceNotFound(device_id))?;
		drop(registry);

		// Get endpoint
		let endpoint = self.endpoint.as_ref().ok_or_else(|| {
			NetworkingError::ConnectionFailed("Endpoint not initialized".to_string())
		})?;

		// Wrap library message in Message envelope
		let message = crate::service::network::protocol::messaging::Message::Library(request);
		let msg_data =
			serde_json::to_vec(&message).map_err(|e| NetworkingError::Serialization(e))?;

		// Connect and send request
		use tokio::io::{AsyncReadExt, AsyncWriteExt};

		let node_addr = iroh::NodeAddr::new(node_id);
		let conn = endpoint
			.connect(node_addr, crate::service::network::core::MESSAGING_ALPN)
			.await
			.map_err(|e| NetworkingError::ConnectionFailed(format!("Failed to connect: {}", e)))?;

		let (mut send, mut recv) = conn.open_bi().await.map_err(|e| {
			NetworkingError::ConnectionFailed(format!("Failed to open stream: {}", e))
		})?;

		// Send request with length prefix
		let len = msg_data.len() as u32;
		send.write_all(&len.to_be_bytes())
			.await
			.map_err(|e| NetworkingError::Transport(format!("Failed to send length: {}", e)))?;
		send.write_all(&msg_data)
			.await
			.map_err(|e| NetworkingError::Transport(format!("Failed to send data: {}", e)))?;
		send.flush()
			.await
			.map_err(|e| NetworkingError::Transport(format!("Failed to flush: {}", e)))?;

		// Read response
		let mut len_buf = [0u8; 4];
		recv.read_exact(&mut len_buf).await.map_err(|e| {
			NetworkingError::Transport(format!("Failed to read response length: {}", e))
		})?;
		let resp_len = u32::from_be_bytes(len_buf) as usize;

		let mut resp_buf = vec![0u8; resp_len];
		recv.read_exact(&mut resp_buf)
			.await
			.map_err(|e| NetworkingError::Transport(format!("Failed to read response: {}", e)))?;

		// Deserialize response
		let response: crate::service::network::protocol::messaging::Message =
			serde_json::from_slice(&resp_buf).map_err(|e| NetworkingError::Serialization(e))?;

		// Extract library message from response
		match response {
			crate::service::network::protocol::messaging::Message::Library(lib_msg) => Ok(lib_msg),
			_ => Err(NetworkingError::Protocol(
				"Unexpected response type".to_string(),
			)),
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

	/// Get the Iroh endpoint for network communication
	pub fn endpoint(&self) -> Option<&Endpoint> {
		self.endpoint.as_ref()
	}

	/// Publish a discovery record for pairing session
	// Note: Discovery for pairing is now handled via mDNS user_data field
	// - Initiator: Sets user_data to session_id via endpoint.set_user_data_for_discovery()
	// - Joiner: Filters endpoint.discovery_stream() for matching session_id in user_data
	// This leverages Iroh's native mDNS capabilities without needing custom key-value storage

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

	/// Strip direct addresses from a NodeAddr to force relay-only connection
	fn strip_direct_addresses(node_addr: NodeAddr) -> NodeAddr {
		use std::collections::BTreeSet;
		NodeAddr::from_parts(
			node_addr.node_id,
			node_addr.relay_url().cloned(),
			BTreeSet::new(), // Empty direct addresses
		)
	}

	/// Connect to a node at a specific address
	///
	/// # Parameters
	/// * `node_addr` - The node address to connect to
	/// * `force_relay` - If true, strip direct addresses and only use relay
	pub async fn connect_to_node(&self, node_addr: NodeAddr, force_relay: bool) -> Result<()> {
		let node_addr = if force_relay {
			Self::strip_direct_addresses(node_addr)
		} else {
			node_addr
		};
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

	/// Try to discover the initiator via mDNS (fast for local networks)
	async fn try_mdns_discovery(&self, session_id: Uuid, force_relay: bool) -> Result<()> {
		use futures::StreamExt;

		let endpoint = self
			.endpoint
			.as_ref()
			.ok_or(NetworkingError::ConnectionFailed(
				"Networking not started".to_string(),
			))?;

		let mut discovery_stream = endpoint.discovery_stream();
		let session_id_str = session_id.to_string();
		let timeout = tokio::time::Duration::from_secs(5); // Shorter timeout for mDNS
		let start = tokio::time::Instant::now();

		self.logger
			.debug(&format!(
				"[mDNS] Looking for pairing session: {}",
				session_id_str
			))
			.await;

		while start.elapsed() < timeout {
			tokio::select! {
				Some(result) = discovery_stream.next() => {
					match result {
						Ok(iroh::discovery::DiscoveryEvent::Discovered(item)) => {
							// Check if this node is broadcasting our session_id
							if let Some(user_data) = item.node_info().data.user_data() {
								if user_data.as_ref() == session_id_str {
									self.logger
										.info(&format!(
											"[mDNS] Found pairing initiator: {} with {} direct addresses",
											item.node_id().fmt_short(),
											item.node_info().data.direct_addresses().len()
										))
										.await;

									// Build NodeAddr from discovery info
									let node_addr = iroh::NodeAddr::from_parts(
										item.node_id(),
										item.node_info().data.relay_url().cloned(),
										item.node_info().data.direct_addresses().clone()
									);

									// Try to connect to the initiator
									if let Err(e) = self.connect_to_node(node_addr.clone(), force_relay).await {
										self.logger
											.warn(&format!("[mDNS] Failed to connect to initiator: {}", e))
											.await;
									} else {
										self.logger.info("[mDNS] Successfully connected to initiator!").await;
										return Ok(());
									}
								}
							}
						}
						Ok(iroh::discovery::DiscoveryEvent::Expired(_)) => {
							// Node expired, continue searching
						}
						Err(e) => {
							self.logger
								.warn(&format!("[mDNS] Discovery stream error: {}", e))
								.await;
						}
					}
				}
				_ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
					// Continue polling
				}
			}
		}

		// mDNS timeout
		Err(NetworkingError::ConnectionFailed(
			"mDNS discovery timeout - initiator not found on local network".to_string(),
		))
	}

	/// Try to discover the initiator via relay (works across networks)
	async fn try_relay_discovery(
		&self,
		pairing_code: &crate::service::network::protocol::pairing::PairingCode,
	) -> Result<()> {
		// Get the NodeId from the pairing code (should always be present in new implementation)
		let node_id = pairing_code.node_id().ok_or_else(|| {
			NetworkingError::ConnectionFailed(
				"Pairing code missing NodeId - this indicates a bug in the new implementation"
					.to_string(),
			)
		})?;

		let relay_url = pairing_code
			.relay_url()
			.map(|url| url.parse::<iroh::RelayUrl>());

		let endpoint = self
			.endpoint
			.as_ref()
			.ok_or(NetworkingError::ConnectionFailed(
				"Networking not started".to_string(),
			))?;

		self.logger
			.info(&format!(
				"[Relay] Attempting to connect to initiator {} via relay",
				node_id.fmt_short()
			))
			.await;

		// Build NodeAddr with relay information
		let relay_url_parsed = if let Some(relay_url) = relay_url {
			match relay_url {
				Ok(url) => {
					self.logger
						.debug(&format!("[Relay] Using relay URL: {}", url))
						.await;
					Some(url)
				}
				Err(e) => {
					self.logger
						.warn(&format!("[Relay] Failed to parse relay URL: {}", e))
						.await;
					None
				}
			}
		} else {
			self.logger
				.warn("[Relay] No relay URL in pairing code, using default relay")
				.await;
			None
		};

		let node_addr = iroh::NodeAddr::from_parts(
			node_id,
			relay_url_parsed,
			vec![], // No direct addresses initially, will use relay
		);

		// Try to connect via relay
		let timeout = tokio::time::Duration::from_secs(10); // Longer timeout for relay
		match tokio::time::timeout(timeout, endpoint.connect(node_addr.clone(), PAIRING_ALPN)).await
		{
			Ok(Ok(conn)) => {
				self.logger
					.info("[Relay] Successfully connected to initiator via relay!")
					.await;

				// Track the connection so it stays alive for the pairing protocol
				{
					let mut connections = self.active_connections.write().await;
					connections.insert(node_id, conn);
					self.logger
						.info(&format!(
							"[Relay] Tracked relay connection to {}",
							node_id.fmt_short()
						))
						.await;
				}

				Ok(())
			}
			Ok(Err(e)) => Err(NetworkingError::ConnectionFailed(format!(
				"Failed to connect via relay: {}",
				e
			))),
			Err(_timeout) => Err(NetworkingError::ConnectionFailed(
				"Relay connection timeout".to_string(),
			)),
		}
	}

	/// Start pairing as an initiator (generates pairing code)
	///
	/// # Parameters
	/// * `force_relay` - If true, only use relay connections (no direct addresses). Useful for testing.
	pub async fn start_pairing_as_initiator(&self, force_relay: bool) -> Result<(String, u32)> {
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

		// Get our node information for relay discovery
		let initiator_node_id = self.node_id();

		// Get our relay URL from the endpoint (wait for relay to connect)
		let relay_url = if let Some(endpoint) = &self.endpoint {
			// Wait for relay to initialize (this is critical!)
			let relay = endpoint.home_relay().initialized().await;
			Some(relay.to_string())
		} else {
			None
		};

		// Generate pairing code with relay information for cross-network pairing
		let random_seed = uuid::Uuid::new_v4();
		let pairing_code =
			crate::service::network::protocol::pairing::PairingCode::from_session_id_with_relay_info(
				random_seed,
				initiator_node_id,
				relay_url,
			);

		// CRITICAL: Use the session_id derived from the pairing code, not the random seed
		// This ensures both initiator and joiner derive the same session_id from the BIP39 words
		let session_id = pairing_code.session_id();

		// Start pairing session with the derived session_id
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

		// Publish pairing session via mDNS using user_data field
		// The joiner will filter discovered nodes by this session_id
		if let Some(endpoint) = &self.endpoint {
			let user_data =
				iroh::node_info::UserData::try_from(session_id.to_string()).map_err(|e| {
					NetworkingError::Protocol(format!("Failed to create user data: {}", e))
				})?;

			self.logger
				.debug(&format!(
					"Setting user_data for discovery: {}",
					user_data.as_ref()
				))
				.await;

			// Get current user_data before setting to verify the change
			let current_node_data = endpoint.node_addr().get();
			self.logger
				.debug(&format!(
					"Current node user_data before set: {:?}",
					current_node_data.as_ref().and_then(|addr| {
						// NodeAddr doesn't expose user_data, so we can't check it here
						Some("(NodeAddr doesn't expose user_data)")
					})
				))
				.await;

			endpoint.set_user_data_for_discovery(Some(user_data.clone()));

			self.logger
				.debug(&format!(
					"Called endpoint.set_user_data_for_discovery with: {}",
					user_data.as_ref()
				))
				.await;

			self.logger
				.info(&format!(
					"Broadcasting pairing session {} via mDNS (set_user_data_for_discovery called)",
					session_id
				))
				.await;

			// Wait for mDNS re-advertisement to propagate
			// When user_data changes, the endpoint triggers a re-publish to discovery services
			// This delay ensures the updated broadcast (with session_id) is sent before joiners start listening
			tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

			self.logger
				.debug("mDNS re-advertisement delay completed, session_id should now be broadcast")
				.await;

			// Log current node address to verify what's being broadcast
			if let Some(node_addr) = self.get_node_addr().ok().flatten() {
				self.logger
					.debug(&format!(
						"Broadcasting with {} direct addresses",
						node_addr.direct_addresses().count()
					))
					.await;
			}
		} else {
			return Err(NetworkingError::Protocol(
				"Networking not started".to_string(),
			));
		}

		let expires_in = 300; // 5 minutes

		Ok((pairing_code.to_string(), expires_in))
	}

	/// Start pairing as a joiner (connects using pairing code string)
	///
	/// # Parameters
	/// * `code` - The BIP39 pairing code
	/// * `force_relay` - If true, only use relay connections (no direct/mDNS). Useful for testing.
	pub async fn start_pairing_as_joiner(&self, code: &str, force_relay: bool) -> Result<()> {
		// Parse BIP39 pairing code
		let pairing_code =
			crate::service::network::protocol::pairing::PairingCode::from_string(code)?;
		self.start_pairing_as_joiner_with_code(pairing_code, force_relay)
			.await
	}

	/// Start pairing as a joiner (connects using parsed pairing code)
	///
	/// # Parameters
	/// * `pairing_code` - The parsed pairing code
	/// * `force_relay` - If true, only use relay connections (no direct/mDNS). Useful for testing.
	pub async fn start_pairing_as_joiner_with_code(
		&self,
		pairing_code: crate::service::network::protocol::pairing::PairingCode,
		force_relay: bool,
	) -> Result<()> {
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

		// Clone pairing code for relay discovery to avoid borrow issues
		let pairing_code_clone = pairing_code.clone();

		// Join pairing session
		pairing_handler
			.join_pairing_session(session_id, pairing_code_clone.clone())
			.await?;

		// Implement dual-path discovery: try mDNS first (fast for local), then relay (for remote)
		// If force_relay is true, skip mDNS and only use relay
		if force_relay {
			self.logger
				.info("Force relay mode: skipping mDNS, using relay only")
				.await;
		} else {
			self.logger
				.info("Starting dual-path discovery: mDNS (local) + Relay (remote)")
				.await;
		}

		let discovery_result = if force_relay {
			// Force relay: only try relay discovery
			match self.try_relay_discovery(&pairing_code_clone).await {
				Ok(()) => {
					self.logger
						.info("Connected via relay (force relay mode)")
						.await;
					Ok(())
				}
				Err(e) => {
					self.logger
						.error(&format!("Relay discovery failed: {}", e))
						.await;
					Err(e)
				}
			}
		} else {
			// Normal mode: race mDNS and relay
			tokio::select! {
				result = self.try_mdns_discovery(session_id, force_relay) => {
					match result {
						Ok(()) => {
							self.logger.info("Connected via mDNS (local network)").await;
							Ok(())
						}
						Err(e) => {
							self.logger.warn(&format!("mDNS discovery failed: {}", e)).await;
							Err(e)
						}
					}
				}
				result = self.try_relay_discovery(&pairing_code_clone) => {
					match result {
						Ok(()) => {
							self.logger.info("Connected via relay (remote network)").await;
							Ok(())
						}
						Err(e) => {
							self.logger.warn(&format!("Relay discovery failed: {}", e)).await;
							Err(e)
						}
					}
				}
			}
		};

		// Handle the discovery result
		match discovery_result {
			Ok(()) => {
				self.logger
					.info("Successfully discovered and connected to initiator!")
					.await;
			}
			Err(e) => {
				self.logger
					.error(&format!("Both mDNS and relay discovery failed: {}", e))
					.await;
				self.logger
					.info("Ensure both devices are on the same network or try again")
					.await;
				return Err(e);
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

	/// Get the PairingCode object for the current session (for generating QR codes)
	/// This is useful for getting the full pairing code with relay info
	pub async fn get_pairing_code_for_current_session(
		&self,
	) -> Result<Option<crate::service::network::protocol::pairing::PairingCode>> {
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

		// Cast to pairing handler
		let pairing_handler = pairing_handler
			.as_any()
			.downcast_ref::<crate::service::network::protocol::PairingProtocolHandler>()
			.ok_or(NetworkingError::Protocol(
				"Invalid pairing handler type".to_string(),
			))?;

		// Get the current pairing code
		Ok(pairing_handler.get_current_pairing_code().await)
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
