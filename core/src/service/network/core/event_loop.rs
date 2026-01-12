//! Networking event loop for handling Iroh connections and messages

use crate::service::network::{
	core::{
		NetworkEvent, FILE_TRANSFER_ALPN, JOB_ACTIVITY_ALPN, MESSAGING_ALPN, PAIRING_ALPN,
		SYNC_ALPN,
	},
	device::DeviceRegistry,
	protocol::ProtocolRegistry,
	utils::{logging::NetworkLogger, NetworkIdentity},
	NetworkingError, Result,
};
use iroh::endpoint::Connection;
use iroh::NodeId;
use iroh::{Endpoint, NodeAddr};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::sync::{broadcast, mpsc, RwLock};
use uuid::Uuid;

/// Commands that can be sent to the event loop
#[derive(Debug)]
pub enum EventLoopCommand {
	// Connection management
	ConnectionEstablished {
		device_id: Uuid,
		node_id: NodeId,
	},
	ConnectionLost {
		device_id: Uuid,
		node_id: NodeId,
		reason: String,
	},
	TrackOutboundConnection {
		node_id: NodeId,
		conn: Connection,
	},

	// Message sending
	SendMessage {
		device_id: Uuid,
		protocol: String,
		data: Vec<u8>,
	},
	SendMessageToNode {
		node_id: NodeId,
		protocol: String,
		data: Vec<u8>,
	},

	// Shutdown
	Shutdown,
}

/// Networking event loop that processes Iroh connections
pub struct NetworkingEventLoop {
	/// Iroh endpoint
	endpoint: Endpoint,

	/// Protocol registry for routing messages
	protocol_registry: Arc<RwLock<ProtocolRegistry>>,

	/// Device registry for managing device state
	device_registry: Arc<RwLock<DeviceRegistry>>,

	/// Event sender for broadcasting network events (broadcast channel allows multiple subscribers)
	event_sender: broadcast::Sender<NetworkEvent>,

	/// Command receiver
	command_rx: mpsc::UnboundedReceiver<EventLoopCommand>,

	/// Command sender (for cloning)
	command_tx: mpsc::UnboundedSender<EventLoopCommand>,

	/// Shutdown receiver
	shutdown_rx: mpsc::UnboundedReceiver<()>,

	/// Shutdown sender (for cloning)
	shutdown_tx: mpsc::UnboundedSender<()>,

	/// Our network identity
	identity: NetworkIdentity,

	/// Active connections tracker (keyed by NodeId and ALPN)
	active_connections: Arc<RwLock<std::collections::HashMap<(NodeId, Vec<u8>), Connection>>>,

	/// Logger for event loop operations
	logger: Arc<dyn NetworkLogger>,
}

impl NetworkingEventLoop {
	/// Create a new networking event loop
	pub fn new(
		endpoint: Endpoint,
		protocol_registry: Arc<RwLock<ProtocolRegistry>>,
		device_registry: Arc<RwLock<DeviceRegistry>>,
		event_sender: broadcast::Sender<NetworkEvent>,
		identity: NetworkIdentity,
		active_connections: Arc<RwLock<std::collections::HashMap<(NodeId, Vec<u8>), Connection>>>,
		logger: Arc<dyn NetworkLogger>,
	) -> Self {
		let (command_tx, command_rx) = mpsc::unbounded_channel();
		let (shutdown_tx, shutdown_rx) = mpsc::unbounded_channel();

		Self {
			endpoint,
			protocol_registry,
			device_registry,
			event_sender,
			command_rx,
			command_tx,
			shutdown_rx,
			shutdown_tx,
			identity,
			active_connections,
			logger,
		}
	}

	/// Get the command sender for sending commands to the event loop
	pub fn command_sender(&self) -> mpsc::UnboundedSender<EventLoopCommand> {
		self.command_tx.clone()
	}

	/// Get the shutdown sender
	pub fn shutdown_sender(&self) -> mpsc::UnboundedSender<()> {
		self.shutdown_tx.clone()
	}

	/// Start the event loop (consumes self)
	pub async fn start(mut self) -> Result<()> {
		// Spawn the event loop task
		let logger = self.logger.clone();
		tokio::spawn(async move {
			if let Err(e) = self.run().await {
				logger
					.error(&format!("Networking event loop error: {}", e))
					.await;
			}
		});

		Ok(())
	}

	/// Main event loop
	async fn run(&mut self) -> Result<()> {
		self.logger.info("Networking event loop started").await;

		// Create interval for connection state monitoring
		let mut connection_monitor_interval =
			tokio::time::interval(tokio::time::Duration::from_secs(10));

		loop {
			tokio::select! {
				// Handle incoming connections
				Some(incoming) = self.endpoint.accept() => {
					let conn = match incoming.await {
						Ok(c) => c,
						Err(e) => {
							self.logger.error(&format!("Failed to establish connection: {}", e)).await;
							continue;
						}
					};

					// Handle the connection based on ALPN
					self.handle_connection(conn).await;
				}

				// Handle commands
				Some(cmd) = self.command_rx.recv() => {
					match cmd {
						EventLoopCommand::Shutdown => {
							self.logger.info("Shutting down networking event loop").await;
							break;
						}
						_ => self.handle_command(cmd).await,
					}
				}

				// Monitor connection state and update DeviceRegistry
				_ = connection_monitor_interval.tick() => {
					self.update_connection_states().await;
				}

				// Handle shutdown signal
				Some(_) = self.shutdown_rx.recv() => {
					self.logger.info("Received shutdown signal").await;
					break;
				}
			}
		}

		self.logger.info("Networking event loop stopped").await;
		Ok(())
	}

	/// Handle an incoming connection
	async fn handle_connection(&self, conn: Connection) {
		// Extract the remote node ID from the connection
		let remote_node_id = match conn.remote_node_id() {
			Ok(key) => key,
			Err(e) => {
				self.logger
					.error(&format!("Failed to get remote node ID: {}", e))
					.await;
				return;
			}
		};

		// Track the connection (keyed by node_id and alpn)
		{
			let alpn_bytes = conn.alpn().unwrap_or_default();
			let mut connections = self.active_connections.write().await;
			connections.insert((remote_node_id, alpn_bytes), conn.clone());
		}

		// For now, we'll need to detect ALPN from the first stream
		// TODO: Find the correct way to get ALPN from iroh Connection
		let alpn = PAIRING_ALPN; // Default to pairing, will be overridden based on stream detection

		self.logger
			.info(&format!("Incoming connection from {:?}", remote_node_id))
			.await;

		// Check if this is a paired device and mark as connected immediately
		let (is_paired_device, paired_device_id) = {
			let registry = self.device_registry.read().await;
			if let Some(device_id) = registry.get_device_by_node(remote_node_id) {
				let state = registry.get_device_state(device_id);
				let is_paired = matches!(
					state,
					Some(crate::service::network::device::DeviceState::Paired { .. })
						| Some(crate::service::network::device::DeviceState::Connected { .. })
						| Some(crate::service::network::device::DeviceState::Disconnected { .. })
				);
				(is_paired, Some(device_id))
			} else {
				(false, None)
			}
		};

		if is_paired_device {
			self.logger
				.info("Paired device connected - marking as connected")
				.await;

			// Mark device as connected immediately when connection arrives
			if let Some(device_id) = paired_device_id {
				let _ = self
					.command_tx
					.send(EventLoopCommand::ConnectionEstablished {
						device_id,
						node_id: remote_node_id,
					});
			}
		}

		self.logger
			.debug("Detecting protocol from incoming streams...")
			.await;

		// Clone necessary components for the spawned task
		let protocol_registry = self.protocol_registry.clone();
		let device_registry = self.device_registry.clone();
		let event_sender = self.event_sender.clone();
		let command_sender = self.command_tx.clone();
		let active_connections = self.active_connections.clone();
		let logger = self.logger.clone();

		// Spawn a task to handle this connection
		tokio::spawn(async move {
			// Handle incoming connection by accepting streams and routing based on content
			Self::handle_incoming_connection(
				conn.clone(),
				protocol_registry,
				device_registry,
				event_sender,
				command_sender,
				remote_node_id,
				logger.clone(),
			)
			.await;

			// Only remove connection if it's actually closed
			if conn.close_reason().is_some() {
				let mut connections = active_connections.write().await;
				let alpn_bytes = conn.alpn().unwrap_or_default();
				connections.remove(&(remote_node_id, alpn_bytes));
				logger
					.info(&format!(
						"Connection to {} removed (closed)",
						remote_node_id
					))
					.await;
			} else {
				logger
					.debug(&format!(
						"Connection to {} still active after stream handling",
						remote_node_id
					))
					.await;
			}
		});
	}

	/// Handle an incoming connection by detecting protocol from streams
	async fn handle_incoming_connection(
		conn: Connection,
		protocol_registry: Arc<RwLock<ProtocolRegistry>>,
		device_registry: Arc<RwLock<DeviceRegistry>>,
		event_sender: broadcast::Sender<NetworkEvent>,
		command_sender: mpsc::UnboundedSender<EventLoopCommand>,
		remote_node_id: NodeId,
		logger: Arc<dyn NetworkLogger>,
	) {
		loop {
			// Try to accept different types of streams
			tokio::select! {
				// Try bidirectional stream (pairing/messaging)
				bi_result = conn.accept_bi() => {
					match bi_result {
						Ok((send, recv)) => {
					// Check if this device is already paired
					let (is_paired, paired_device_id) = {
						let registry = device_registry.read().await;
						if let Some(device_id) = registry.get_device_by_node(remote_node_id) {
							let state = registry.get_device_state(device_id);
							let paired = match state {
								Some(crate::service::network::device::DeviceState::Paired { .. }) |
								Some(crate::service::network::device::DeviceState::Connected { .. }) |
								Some(crate::service::network::device::DeviceState::Disconnected { .. }) => true,
								_ => false,
							};
							(paired, Some(device_id))
						} else {
							(false, None)
						}
					};

					// If this is a paired device connecting to us, mark it as connected
					if is_paired {
						if let Some(device_id) = paired_device_id {
							let _ = command_sender.send(EventLoopCommand::ConnectionEstablished {
								device_id,
								node_id: remote_node_id,
							});
						}
					}

					// Route to handler based on ALPN
					let alpn_bytes = conn.alpn().unwrap_or_default();

					if alpn_bytes == MESSAGING_ALPN {
						let registry = protocol_registry.read().await;
						if let Some(handler) = registry.get_handler("messaging") {
							logger.info("Routing to messaging handler (ALPN match)").await;
							handler
								.handle_stream(Box::new(send), Box::new(recv), remote_node_id)
								.await;
						}
						continue;
					} else if alpn_bytes == PAIRING_ALPN {
						let registry = protocol_registry.read().await;
						if let Some(handler) = registry.get_handler("pairing") {
							logger.info("Routing to pairing handler (ALPN match)").await;
							handler
								.handle_stream(Box::new(send), Box::new(recv), remote_node_id)
								.await;
						}
						continue;
					} else if alpn_bytes == FILE_TRANSFER_ALPN {
						let registry = protocol_registry.read().await;
						if let Some(handler) = registry.get_handler("file_transfer") {
							logger.info("Routing to file_transfer handler (ALPN match)").await;
							handler
								.handle_stream(Box::new(send), Box::new(recv), remote_node_id)
								.await;
						}
						continue;
					} else if alpn_bytes == SYNC_ALPN {
						let registry = protocol_registry.read().await;
						if let Some(handler) = registry.get_handler("sync") {
							handler
								.handle_stream(Box::new(send), Box::new(recv), remote_node_id)
								.await;
						}
						continue;
					} else if alpn_bytes == JOB_ACTIVITY_ALPN {
						let registry = protocol_registry.read().await;
						if let Some(handler) = registry.get_handler("job_activity") {
							logger.info("Routing to job_activity handler (ALPN match)").await;
							handler
								.handle_stream(Box::new(send), Box::new(recv), remote_node_id)
								.await;
						}
						continue;
					} else {
						logger
							.warn(&format!(
								"Unknown ALPN: {:?}",
								String::from_utf8_lossy(&alpn_bytes)
							))
							.await;
						continue;
					}
				}
					Err(e) => {
						// Check if the QUIC connection itself is closed
						if conn.close_reason().is_some() {
							logger.info(&format!("Connection closed: {:?}", conn.close_reason())).await;

							// Fire ConnectionLost event if this was a paired device
							let registry = device_registry.read().await;
							if let Some(device_id) = registry.get_device_by_node(remote_node_id) {
								logger.info(&format!("Connection lost for device {} - connection closed", device_id)).await;
								let _ = command_sender.send(EventLoopCommand::ConnectionLost {
									device_id,
									node_id: remote_node_id,
									reason: "Connection closed".to_string(),
								});
							}
							break;
						} else {
							// Stream error but connection still alive - just continue accepting
							logger.debug(&format!("No more streams to accept ({}), but connection still alive", e)).await;
							// Don't break - connection is still valid, just no streams right now
						}
					}
					}
				}
				// Try unidirectional stream (file transfer)
				uni_result = conn.accept_uni() => {
					match uni_result {
						Ok(recv) => {
							logger.info(&format!("Accepted unidirectional stream from {}", remote_node_id)).await;

							// Get ALPN to determine which protocol handler to use
							let alpn_bytes = conn.alpn().unwrap_or_default();
							let registry = protocol_registry.read().await;

							// Route based on ALPN
							if alpn_bytes == SYNC_ALPN {
								if let Some(handler) = registry.get_handler("sync") {
									logger.info("Directing unidirectional stream to sync handler").await;
									handler.handle_stream(
										Box::new(tokio::io::empty()), // No send stream for unidirectional
										Box::new(recv),
										remote_node_id,
									).await;
								}
							} else if alpn_bytes == FILE_TRANSFER_ALPN {
								if let Some(handler) = registry.get_handler("file_transfer") {
									logger.debug("Directing unidirectional stream to file transfer handler").await;
									handler.handle_stream(
										Box::new(tokio::io::empty()), // No send stream for unidirectional
										Box::new(recv),
										remote_node_id,
									).await;
								}
							} else {
								logger.debug(&format!("Unknown ALPN for unidirectional stream: {:?}", alpn_bytes)).await;
							}
						}
					Err(e) => {
						// Check if the QUIC connection itself is closed
						if conn.close_reason().is_some() {
							logger.info(&format!("Connection closed: {:?}", conn.close_reason())).await;

							// Fire ConnectionLost event if this was a paired device
							let registry = device_registry.read().await;
							if let Some(device_id) = registry.get_device_by_node(remote_node_id) {
								logger.info(&format!("Connection lost for device {} - connection closed", device_id)).await;
								let _ = command_sender.send(EventLoopCommand::ConnectionLost {
									device_id,
									node_id: remote_node_id,
									reason: "Connection closed".to_string(),
								});
							}
							break;
						} else {
							// Stream error but connection still alive - just continue accepting
							logger.debug(&format!("No more streams to accept ({}), but connection still alive", e)).await;
							// Don't break - connection is still valid, just no streams right now
						}
					}
					}
				}
			}
		}
	}

	/// Handle a command from the main thread
	async fn handle_command(&self, command: EventLoopCommand) {
		match command {
			EventLoopCommand::ConnectionEstablished { device_id, node_id } => {
				// Update device registry
				let mut registry = self.device_registry.write().await;
				if let Err(e) = registry.set_device_connected(device_id, node_id).await {
					self.logger
						.error(&format!("Failed to update device connection state: {}", e))
						.await;
				}

				// Send connection event
				let _ = self
					.event_sender
					.send(NetworkEvent::ConnectionEstablished { device_id, node_id });
			}

			EventLoopCommand::SendMessage {
				device_id,
				protocol,
				data,
			} => {
				// Look up node ID for device
				let node_id = {
					let registry = self.device_registry.read().await;
					registry.get_node_id_for_device(device_id)
				};

				if let Some(node_id) = node_id {
					// Send to node
					self.send_to_node(node_id, &protocol, data).await;
				} else {
					self.logger
						.warn(&format!("No node ID found for device {}", device_id))
						.await;
				}
			}

			EventLoopCommand::SendMessageToNode {
				node_id,
				protocol,
				data,
			} => {
				self.send_to_node(node_id, &protocol, data).await;
			}

			EventLoopCommand::ConnectionLost {
				device_id,
				node_id,
				reason,
			} => {
				// Check if device is already disconnected to prevent infinite loops
				let should_process = {
					let registry = self.device_registry.read().await;
					if let Some(state) = registry.get_device_state(device_id) {
						// Only process if device is Connected or Paired (not already Disconnected)
						matches!(
							state,
							crate::service::network::device::DeviceState::Connected { .. }
								| crate::service::network::device::DeviceState::Paired { .. }
						)
					} else {
						false
					}
				};

				if !should_process {
					self.logger
						.debug(&format!(
						"Ignoring duplicate ConnectionLost for device {} (already disconnected)",
						device_id
					))
						.await;
					return;
				}

				self.logger
					.info(&format!(
						"Connection lost to device {} (node: {}): {}",
						device_id, node_id, reason
					))
					.await;

				// Remove from active connections
				{
					let mut connections = self.active_connections.write().await;
					connections.retain(|(nid, _alpn), _conn| *nid != node_id);
				}

				// Update device registry to mark as disconnected
				let mut registry = self.device_registry.write().await;
				if let Err(e) = registry
					.mark_disconnected(
						device_id,
						crate::service::network::device::DisconnectionReason::ConnectionLost,
					)
					.await
				{
					self.logger
						.warn(&format!(
							"Could not mark device {} as disconnected: {}",
							device_id, e
						))
						.await;
					// Don't trigger reconnection if we couldn't mark as disconnected
					return;
				}

				// Send connection lost event
				let _ = self
					.event_sender
					.send(NetworkEvent::ConnectionLost { device_id, node_id });

				// Trigger immediate reconnection attempt with exponential backoff
				self.logger
					.info(&format!(
						"Triggering reconnection attempt for device {}",
						device_id
					))
					.await;

				// Get device info for reconnection
				if let Ok(auto_reconnect_devices) = registry.get_auto_reconnect_devices().await {
					if let Some((_, persisted_device)) = auto_reconnect_devices
						.into_iter()
						.find(|(id, _)| *id == device_id)
					{
						let command_sender = Some(self.command_tx.clone());
						let endpoint = Some(self.endpoint.clone());
						let logger = self.logger.clone();

						// Spawn reconnection with a small delay to prevent immediate retry loops
						tokio::spawn(async move {
							// Wait 2 seconds before attempting reconnection to avoid tight loop
							tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

							crate::service::network::core::NetworkingService::attempt_device_reconnection(
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
			}

			EventLoopCommand::TrackOutboundConnection { node_id, conn } => {
				// Add outbound connection to active connections map
				let alpn_bytes = conn.alpn().unwrap_or_default();
				{
					let mut connections = self.active_connections.write().await;
					connections.insert((node_id, alpn_bytes.clone()), conn.clone());
				}

				self.logger
					.info(&format!(
						"Tracking outbound connection to {} (ALPN: {:?}), spawning stream handler",
						node_id,
						String::from_utf8_lossy(&alpn_bytes)
					))
					.await;

				// Spawn handler task to accept incoming streams on this outbound connection
				// This is critical - Iroh connections are bidirectional, so we need to listen
				// for streams even on connections we initiated
				let protocol_registry = self.protocol_registry.clone();
				let device_registry = self.device_registry.clone();
				let event_sender = self.event_sender.clone();
				let command_sender = self.command_tx.clone();
				let logger = self.logger.clone();
				let active_connections = self.active_connections.clone();

				tokio::spawn(async move {
					Self::handle_incoming_connection(
						conn.clone(),
						protocol_registry,
						device_registry,
						event_sender,
						command_sender,
						node_id,
						logger.clone(),
					)
					.await;

					// Clean up when handler exits
					if conn.close_reason().is_some() {
						let mut connections = active_connections.write().await;
						connections.remove(&(node_id, alpn_bytes));
						logger
							.info(&format!(
								"Outbound connection to {} closed and removed",
								node_id
							))
							.await;
					}
				});
			}

			EventLoopCommand::Shutdown => {
				// Handled in main loop
			}
		}
	}

	/// Send a message to a specific node
	async fn send_to_node(&self, node_id: NodeId, protocol: &str, data: Vec<u8>) {
		self.logger
			.debug(&format!(
				"Sending {} message to {} ({} bytes)",
				protocol,
				node_id,
				data.len()
			))
			.await;

		// Determine ALPN based on protocol
		let alpn = match protocol {
			"pairing" => PAIRING_ALPN,
			"file_transfer" => {
				self.logger
					.debug(&format!(
						"Using ALPN: {:?}",
						String::from_utf8_lossy(FILE_TRANSFER_ALPN)
					))
					.await;
				FILE_TRANSFER_ALPN
			}
			"messaging" => MESSAGING_ALPN,
			_ => {
				self.logger
					.error(&format!("Unknown protocol: {}", protocol))
					.await;
				return;
			}
		};

		// Create node address (Iroh will use existing connection if available)
		let node_addr = NodeAddr::new(node_id);

		// Connect with specific ALPN
		self.logger
			.debug(&format!(
				"Attempting to connect to {} with ALPN: {:?}",
				node_id,
				String::from_utf8_lossy(alpn)
			))
			.await;
		match self.endpoint.connect(node_addr, alpn).await {
			Ok(conn) => {
				self.logger
					.debug(&format!(
						"Successfully connected to {} with ALPN: {:?}",
						node_id,
						String::from_utf8_lossy(alpn)
					))
					.await;
				// Track the connection
				{
					let mut connections = self.active_connections.write().await;
					let alpn_bytes = conn.alpn().unwrap_or_default();
					connections.insert((node_id, alpn_bytes), conn.clone());
				}

				// Open appropriate stream based on protocol
				match protocol {
					"pairing" | "messaging" => {
						// Bidirectional stream
						match conn.open_bi().await {
							Ok((mut send, _recv)) => {
								// For pairing, send with length prefix like send_pairing_message_to_node does
								if protocol == "pairing" {
									self.logger
										.info(&format!(
											"Sending pairing message to {} ({} bytes)",
											node_id,
											data.len()
										))
										.await;

									// Send message length first
									let len = data.len() as u32;
									if let Err(e) = send.write_all(&len.to_be_bytes()).await {
										self.logger
											.error(&format!(
												"Failed to write pairing message length: {}",
												e
											))
											.await;
										return;
									}
								}

								// Send the data
								if let Err(e) = send.write_all(&data).await {
									self.logger
										.error(&format!(
											"Failed to send {} message: {}",
											protocol, e
										))
										.await;
								} else {
									self.logger
										.info(&format!(
											"Successfully sent {} message to {}",
											protocol, node_id
										))
										.await;
								}

								// Flush the stream to ensure data is sent
								if let Err(e) = send.flush().await {
									self.logger
										.error(&format!(
											"Failed to flush {} stream: {}",
											protocol, e
										))
										.await;
								}

								let _ = send.finish();
							}
							Err(e) => {
								self.logger
									.error(&format!("Failed to open {} stream: {}", protocol, e))
									.await;
							}
						}
					}
					"file_transfer" => {
						// Unidirectional stream
						self.logger
							.debug(&format!("Opening unidirectional stream to {}", node_id))
							.await;
						match conn.open_uni().await {
							Ok(mut send) => {
								self.logger.debug("Opened stream, sending data").await;
								// Send with the expected format for file transfer protocol
								// Transfer type: 0 for file metadata request
								if let Err(e) = send.write_all(&[0u8]).await {
									self.logger
										.error(&format!(
											"Failed to write file transfer type: {}",
											e
										))
										.await;
									return;
								}

								// Send message length (big-endian u32)
								let len = data.len() as u32;
								if let Err(e) = send.write_all(&len.to_be_bytes()).await {
									self.logger
										.error(&format!(
											"Failed to write file transfer message length: {}",
											e
										))
										.await;
									return;
								}

								// Send the actual message data
								if let Err(e) = send.write_all(&data).await {
									self.logger
										.error(&format!("Failed to send file transfer data: {}", e))
										.await;
								} else {
									self.logger
										.debug(&format!("Successfully sent {} bytes", data.len()))
										.await;
								}
								let _ = send.finish();
							}
							Err(e) => {
								self.logger
									.error(&format!("Failed to open stream: {}", e))
									.await;
							}
						}
					}
					_ => {}
				}
			}
			Err(e) => {
				self.logger
					.error(&format!("Failed to connect to {}: {}", node_id, e))
					.await;
			}
		}
	}

	/// Update DeviceRegistry connection states based on Iroh's remote_info
	///
	/// This monitors Iroh connections and updates the DeviceRegistry state accordingly.
	/// Devices transition to Connected when Iroh reports an active connection, and back
	/// to Paired when the connection is lost. This is cosmetic only - sync routing uses
	/// is_node_connected() which queries Iroh directly.
	async fn update_connection_states(&self) {
		// Get all remote info from Iroh
		let remote_infos: Vec<_> = self.endpoint.remote_info_iter().collect();

		// Lock registry for updates
		let mut registry = self.device_registry.write().await;

		// Track which node IDs Iroh reports as connected
		let mut connected_node_ids = std::collections::HashSet::new();

		// Update devices that Iroh reports as connected
		for remote_info in remote_infos {
			// Check if this is an active connection
			let is_connected =
				!matches!(remote_info.conn_type, iroh::endpoint::ConnectionType::None);

			if is_connected {
				connected_node_ids.insert(remote_info.node_id);

				// Find device for this node
				if let Some(device_id) = registry.get_device_by_node_id(remote_info.node_id) {
					// Update to Connected state if not already
					if let Err(e) = registry
						.update_device_from_connection(
							device_id,
							remote_info.node_id,
							remote_info.conn_type,
							remote_info.latency,
						)
						.await
					{
						self.logger
							.debug(&format!(
								"Failed to update device {} connection state: {}",
								device_id, e
							))
							.await;
					}
				}
			}
		}

		// Check devices that are marked as Connected in registry but NOT in Iroh's list
		// These devices have silently disconnected and need to be transitioned back to Paired
		let all_devices = registry.get_all_devices();
		for (device_id, state) in all_devices {
			if let crate::service::network::device::DeviceState::Connected { info, .. } = state {
				// Get the node_id for this device
				if let Ok(node_id) = info.network_fingerprint.node_id.parse::<NodeId>() {
					// If this node is NOT in Iroh's connected list, it's stale
					if !connected_node_ids.contains(&node_id) {
						self.logger
							.info(&format!(
								"Device {} ({}) is marked Connected but not in Iroh's connection list - transitioning to Paired",
								device_id, info.device_name
							))
							.await;

						// Transition to Paired state via update_device_from_connection with None conn_type
						if let Err(e) = registry
							.update_device_from_connection(
								device_id,
								node_id,
								iroh::endpoint::ConnectionType::None,
								None,
							)
							.await
						{
							self.logger
								.warn(&format!(
									"Failed to transition stale device {} to Paired: {}",
									device_id, e
								))
								.await;
						}
					}
				}
			}
		}
	}
}
