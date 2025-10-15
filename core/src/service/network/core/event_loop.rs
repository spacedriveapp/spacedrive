//! Networking event loop for handling Iroh connections and messages

use crate::service::network::{
	core::{NetworkEvent, FILE_TRANSFER_ALPN, MESSAGING_ALPN, PAIRING_ALPN},
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
		event_sender: broadcast::Sender<NetworkEvent>,
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
		self.logger.info("Networking event loop started").await;

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
		let remote_node_id = match conn.remote_node_id() {
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
							logger.info(&format!("Accepted bidirectional stream from {}", remote_node_id)).await;

					// Check if this device is already paired
					let (is_paired, paired_device_id) = {
						let registry = device_registry.read().await;
						if let Some(device_id) = registry.get_device_by_node(remote_node_id) {
							let state = registry.get_device_state(device_id);
							logger.debug(&format!(
								"Found device {} for node {}, state: {:?}",
								device_id,
								remote_node_id,
								state.as_ref().map(|s| format!("{:?}", s))
							)).await;
							let paired = match state {
								Some(crate::service::network::device::DeviceState::Paired { .. }) |
								Some(crate::service::network::device::DeviceState::Connected { .. }) |
								Some(crate::service::network::device::DeviceState::Disconnected { .. }) => true,
								_ => false,
							};
							(paired, Some(device_id))
						} else {
							logger.debug(&format!("No device found for node {}", remote_node_id)).await;
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

					// For paired devices, check if this is a file transfer stream
					// File transfer streams start with type byte 0, followed by length and message
					if is_paired {
						// Try to peek at the first byte to detect file transfer protocol
						// File transfer protocol starts with transfer_type byte
						use tokio::io::AsyncReadExt;
						let mut peek_buf = [0u8; 1];
						let mut recv_peekable = recv;

						// Try to peek the first byte
						let bytes_read = recv_peekable.read(&mut peek_buf).await;
						match bytes_read {
							Ok(Some(n)) if n > 0 => {
									// Check if this looks like a file transfer message (type 0 or 1)
									if peek_buf[0] <= 1 {
									// Likely file transfer - but we need to put the byte back
									// Wrap in a custom reader that replays this byte
									struct PrependReader<R> {
										byte: Option<u8>,
										inner: R,
									}

									impl<R: tokio::io::AsyncRead + Unpin> tokio::io::AsyncRead for PrependReader<R> {
										fn poll_read(
											mut self: std::pin::Pin<&mut Self>,
											cx: &mut std::task::Context<'_>,
											buf: &mut tokio::io::ReadBuf<'_>,
										) -> std::task::Poll<std::io::Result<()>> {
											if let Some(byte) = self.byte.take() {
												buf.put_slice(&[byte]);
												std::task::Poll::Ready(Ok(()))
											} else {
												std::pin::Pin::new(&mut self.inner).poll_read(cx, buf)
											}
										}
									}

									let recv_with_byte = PrependReader {
										byte: Some(peek_buf[0]),
										inner: recv_peekable,
									};

									let registry = protocol_registry.read().await;
									if let Some(handler) = registry.get_handler("file_transfer") {
										logger.info("Directing bidirectional stream to file_transfer handler").await;
										handler.handle_stream(
											Box::new(send),
											Box::new(recv_with_byte),
											remote_node_id,
										).await;
										logger.info("file_transfer handler completed for stream").await;
									}
									continue; // Skip the default handler logic below
								} else {
									// Not file transfer, use messaging
									// Need to wrap to replay the byte
									struct PrependReader<R> {
										byte: Option<u8>,
										inner: R,
									}

									impl<R: tokio::io::AsyncRead + Unpin> tokio::io::AsyncRead for PrependReader<R> {
										fn poll_read(
											mut self: std::pin::Pin<&mut Self>,
											cx: &mut std::task::Context<'_>,
											buf: &mut tokio::io::ReadBuf<'_>,
										) -> std::task::Poll<std::io::Result<()>> {
											if let Some(byte) = self.byte.take() {
												buf.put_slice(&[byte]);
												std::task::Poll::Ready(Ok(()))
											} else {
												std::pin::Pin::new(&mut self.inner).poll_read(cx, buf)
											}
										}
									}

									let recv_with_byte = PrependReader {
										byte: Some(peek_buf[0]),
										inner: recv_peekable,
									};

									let registry = protocol_registry.read().await;
									if let Some(handler) = registry.get_handler("messaging") {
										logger.info("Directing bidirectional stream to messaging handler").await;
										handler.handle_stream(
											Box::new(send),
											Box::new(recv_with_byte),
											remote_node_id,
										).await;
										logger.info("messaging handler completed for stream").await;
									}
									continue; // Skip the default handler logic below
								}
							}
							_ => {
								// Default to messaging if we can't peek, no bytes, or error
								// Use recv_peekable since recv was already moved
								let registry = protocol_registry.read().await;
								if let Some(handler) = registry.get_handler("messaging") {
									logger.info("Directing bidirectional stream to messaging handler (fallback)").await;
									handler.handle_stream(
										Box::new(send),
										Box::new(recv_peekable),
										remote_node_id,
									).await;
									logger.info("messaging handler completed for stream").await;
								}
								continue;
							}
						}
					} else {
						// Unpaired device, use pairing handler
						let registry = protocol_registry.read().await;
						if let Some(handler) = registry.get_handler("pairing") {
							logger.info("Directing bidirectional stream to pairing handler").await;
							handler.handle_stream(
								Box::new(send),
								Box::new(recv),
								remote_node_id,
							).await;
							logger.info("pairing handler completed for stream").await;
						} else {
							logger.error("No pairing handler registered!").await;
						}
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
				// Get the address from the active connection map
				let connections = self.active_connections.read().await;
				let addresses = if let Some(_conn) = connections.get(&node_id) {
					vec!["connected".to_string()] // TODO: Find equivalent of remote_address() in iroh 0.91
				} else {
					self.logger
						.warn(&format!(
							"Could not find active connection for node {}",
							node_id
						))
						.await;
					vec![]
				};
				drop(connections);

				// Update device registry
				let mut registry = self.device_registry.write().await;
				if let Err(e) = registry
					.set_device_connected(device_id, node_id, addresses)
					.await
				{
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
					connections.remove(&node_id);
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
				let mut connections = self.active_connections.write().await;
				connections.insert(node_id, conn);
				self.logger
					.debug(&format!("Tracked outbound connection to {}", node_id))
					.await;
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
}
