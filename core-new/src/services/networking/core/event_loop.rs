//! Networking event loop for handling Iroh connections and messages

use crate::services::networking::{
	core::{NetworkEvent, FILE_TRANSFER_ALPN, MESSAGING_ALPN, PAIRING_ALPN},
	device::DeviceRegistry,
	protocols::ProtocolRegistry,
	utils::{logging::NetworkLogger, NetworkIdentity},
	NetworkingError, Result,
};
use iroh::net::endpoint::Connection;
use iroh::net::key::NodeId;
use iroh::net::{Endpoint, NodeAddr};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

/// Commands that can be sent to the event loop
#[derive(Debug)]
pub enum EventLoopCommand {
	// Connection management
	ConnectionEstablished {
		device_id: Uuid,
		node_id: NodeId,
	},
	EstablishPersistentConnection {
		device_id: Uuid,
		node_id: NodeId,
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

	/// Event sender for broadcasting network events
	event_sender: mpsc::UnboundedSender<NetworkEvent>,

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

	/// Active connections tracker
	active_connections: Arc<RwLock<std::collections::HashMap<NodeId, Connection>>>,

	/// Logger for event loop operations
	logger: Arc<dyn NetworkLogger>,
}

impl NetworkingEventLoop {
	/// Create a new networking event loop
	pub fn new(
		endpoint: Endpoint,
		protocol_registry: Arc<RwLock<ProtocolRegistry>>,
		device_registry: Arc<RwLock<DeviceRegistry>>,
		event_sender: mpsc::UnboundedSender<NetworkEvent>,
		identity: NetworkIdentity,
		active_connections: Arc<RwLock<std::collections::HashMap<NodeId, Connection>>>,
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
		self.logger.info("🚀 Networking event loop started").await;

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
		let remote_node_id = match iroh::net::endpoint::get_remote_node_id(&conn) {
			Ok(key) => key,
			Err(e) => {
				self.logger
					.error(&format!("Failed to get remote node ID: {}", e))
					.await;
				return;
			}
		};

		// Track the connection
		{
			let mut connections = self.active_connections.write().await;
			connections.insert(remote_node_id, conn.clone());
		}

		// For now, we'll need to detect ALPN from the first stream
		// TODO: Find the correct way to get ALPN from iroh Connection
		let alpn = PAIRING_ALPN; // Default to pairing, will be overridden based on stream detection

		self.logger
			.info(&format!("Incoming connection from {:?}", remote_node_id))
			.await;
		self.logger
			.debug("Detecting protocol from incoming streams...")
			.await;

		// Clone necessary components for the spawned task
		let protocol_registry = self.protocol_registry.clone();
		let device_registry = self.device_registry.clone();
		let event_sender = self.event_sender.clone();
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
				remote_node_id,
				logger.clone(),
			)
			.await;

			// Only remove connection if it's actually closed
			if conn.close_reason().is_some() {
				let mut connections = active_connections.write().await;
				connections.remove(&remote_node_id);
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
		event_sender: mpsc::UnboundedSender<NetworkEvent>,
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
							logger.info(&format!("Accepted bidirectional stream from {}", remote_node_id)).await;
							
							// Check if this device is already paired
							let is_paired = {
								let registry = device_registry.read().await;
								if let Some(device_id) = registry.get_device_by_node(remote_node_id) {
									match registry.get_device_state(device_id) {
										Some(crate::services::networking::device::DeviceState::Paired { .. }) |
										Some(crate::services::networking::device::DeviceState::Connected { .. }) => true,
										_ => false,
									}
								} else {
									false
								}
							};
							
							// Route to appropriate handler based on pairing status
							let handler_name = if is_paired { "messaging" } else { "pairing" };
							
							let registry = protocol_registry.read().await;
							if let Some(handler) = registry.get_handler(handler_name) {
								logger.info(&format!("Directing bidirectional stream to {} handler", handler_name)).await;
								handler.handle_stream(
									Box::new(send),
									Box::new(recv),
									remote_node_id,
								).await;
								logger.info(&format!("{} handler completed for stream from {}", handler_name, remote_node_id)).await;
							} else {
								logger.error(&format!("No {} handler registered!", handler_name)).await;
							}
						}
						Err(e) => {
							logger.error(&format!("Failed to accept bidirectional stream: {}", e)).await;
							break;
						}
					}
				}
				// Try unidirectional stream (file transfer)
				uni_result = conn.accept_uni() => {
					match uni_result {
						Ok(recv) => {
							logger.debug(&format!("Accepted unidirectional stream from {}", remote_node_id)).await;
							// Unidirectional streams are for file transfer
							let registry = protocol_registry.read().await;
							if let Some(handler) = registry.get_handler("file_transfer") {
								logger.debug("Directing unidirectional stream to file transfer handler").await;
								handler.handle_stream(
									Box::new(tokio::io::empty()), // No send stream for unidirectional
									Box::new(recv),
									remote_node_id,
								).await;
							}
						}
						Err(e) => {
							logger.error(&format!("Failed to accept unidirectional stream: {}", e)).await;
							break;
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
				// For now, we don't have the remote device's addresses here
				// They should be discovered through the discovery service
				let addresses = vec![];
				
				// Update device registry
				let mut registry = self.device_registry.write().await;
				if let Err(e) = registry.set_device_connected(device_id, node_id, addresses).await {
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

			EventLoopCommand::EstablishPersistentConnection { device_id, node_id } => {
				// Establish a new persistent connection using MESSAGING_ALPN
				self.logger
					.info(&format!(
						"Establishing persistent messaging connection to device {} (node: {})",
						device_id, node_id
					))
					.await;

				// Create NodeAddr for the connection
				let node_addr = NodeAddr::new(node_id);
				
				// Attempt to connect with MESSAGING_ALPN
				match self.endpoint.connect(node_addr, MESSAGING_ALPN).await {
					Ok(conn) => {
						// Store the connection
						{
							let mut connections = self.active_connections.write().await;
							connections.insert(node_id, conn);
						}
						
						self.logger
							.info(&format!(
								"Successfully established persistent connection to device {} (node: {})",
								device_id, node_id
							))
							.await;
						
						// For now, we don't have the remote device's addresses here
						// They should be discovered through the discovery service
						let addresses = vec![];
						
						// Update device registry to mark as connected
						let mut registry = self.device_registry.write().await;
						if let Err(e) = registry.set_device_connected(device_id, node_id, addresses).await {
							self.logger
								.error(&format!("Failed to update device connection state: {}", e))
								.await;
						}
						
						// Send connection event
						let _ = self
							.event_sender
							.send(NetworkEvent::ConnectionEstablished { device_id, node_id });
					}
					Err(e) => {
						self.logger
							.error(&format!(
								"Failed to establish persistent connection to device {} (node: {}): {}",
								device_id, node_id, e
							))
							.await;
					}
				}
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
					connections.insert(node_id, conn.clone());
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
											node_id, data.len()
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
}
