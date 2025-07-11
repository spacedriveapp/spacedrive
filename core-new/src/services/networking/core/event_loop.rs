//! Networking event loop for handling Iroh connections and messages

use crate::services::networking::{
	core::{NetworkEvent, PAIRING_ALPN, FILE_TRANSFER_ALPN, MESSAGING_ALPN},
	device::DeviceRegistry,
	protocols::ProtocolRegistry,
	utils::NetworkIdentity,
	NetworkingError, Result,
};
use iroh::net::{Endpoint, NodeAddr};
use iroh::net::key::NodeId;
use iroh::net::endpoint::Connection;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

/// Commands that can be sent to the event loop
#[derive(Debug)]
pub enum EventLoopCommand {
	// Connection management
	ConnectionEstablished {
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
		tokio::spawn(async move {
			if let Err(e) = self.run().await {
				eprintln!("Networking event loop error: {}", e);
			}
		});
		
		Ok(())
	}
	
	/// Main event loop
	async fn run(&mut self) -> Result<()> {
		println!("🚀 Networking event loop started");
		
		loop {
			tokio::select! {
				// Handle incoming connections
				Some(incoming) = self.endpoint.accept() => {
					let conn = match incoming.await {
						Ok(c) => c,
						Err(e) => {
							eprintln!("Failed to establish connection: {}", e);
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
							println!("Shutting down networking event loop");
							break;
						}
						_ => self.handle_command(cmd).await,
					}
				}
				
				// Handle shutdown signal
				Some(_) = self.shutdown_rx.recv() => {
					println!("Received shutdown signal");
					break;
				}
			}
		}
		
		println!("🛑 Networking event loop stopped");
		Ok(())
	}
	
	/// Handle an incoming connection
	async fn handle_connection(&self, conn: Connection) {
		// Extract the remote node ID from the connection
		let remote_node_id = match iroh::net::endpoint::get_remote_node_id(&conn) {
			Ok(key) => key,
			Err(e) => {
				eprintln!("Failed to get remote node ID: {}", e);
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
		
		println!("📥 Incoming connection from {:?}", remote_node_id);
		println!("🔍 ROUTING: Detecting protocol from incoming streams...");
		
		// Clone necessary components for the spawned task
		let protocol_registry = self.protocol_registry.clone();
		let device_registry = self.device_registry.clone();
		let event_sender = self.event_sender.clone();
		let active_connections = self.active_connections.clone();
		
		// Spawn a task to handle this connection
		tokio::spawn(async move {
			// Handle incoming connection by accepting streams and routing based on content
			Self::handle_incoming_connection(
				conn.clone(),
				protocol_registry,
				device_registry,
				event_sender,
				remote_node_id,
			).await;
			
			// Only remove connection if it's actually closed
			if conn.close_reason().is_some() {
				let mut connections = active_connections.write().await;
				connections.remove(&remote_node_id);
				println!("🔌 Connection to {} removed (closed)", remote_node_id);
			} else {
				println!("🔌 Connection to {} still active after stream handling", remote_node_id);
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
	) {
		loop {
			// Try to accept different types of streams
			tokio::select! {
				// Try bidirectional stream (pairing/messaging)
				bi_result = conn.accept_bi() => {
					match bi_result {
						Ok((send, recv)) => {
							println!("📥 Accepted bidirectional stream from {}", remote_node_id);
							// For now, assume bidirectional streams are pairing
							let registry = protocol_registry.read().await;
							if let Some(handler) = registry.get_handler("pairing") {
								println!("🔀 ROUTING: Directing bidirectional stream to pairing handler");
								handler.handle_stream(
									Box::new(send),
									Box::new(recv),
									remote_node_id,
								).await;
							}
						}
						Err(e) => {
							println!("Failed to accept bidirectional stream: {}", e);
							break;
						}
					}
				}
				// Try unidirectional stream (file transfer)  
				uni_result = conn.accept_uni() => {
					match uni_result {
						Ok(recv) => {
							println!("📥 Accepted unidirectional stream from {}", remote_node_id);
							// Unidirectional streams are for file transfer
							let registry = protocol_registry.read().await;
							if let Some(handler) = registry.get_handler("file_transfer") {
								println!("🔀 ROUTING: Directing unidirectional stream to file transfer handler");
								handler.handle_stream(
									Box::new(tokio::io::empty()), // No send stream for unidirectional
									Box::new(recv),
									remote_node_id,
								).await;
							}
						}
						Err(e) => {
							println!("Failed to accept unidirectional stream: {}", e);
							break;
						}
					}
				}
			}
		}
	}
	
	/// Handle a pairing connection
	async fn handle_pairing_connection(
		conn: Connection,
		protocol_registry: Arc<RwLock<ProtocolRegistry>>,
		device_registry: Arc<RwLock<DeviceRegistry>>,
		event_sender: mpsc::UnboundedSender<NetworkEvent>,
		remote_node_id: NodeId,
	) {
		// Wait for incoming streams with a timeout
		let timeout_duration = std::time::Duration::from_secs(30); // 30 second timeout
		
		loop {
			match tokio::time::timeout(timeout_duration, conn.accept_bi()).await {
				Ok(Ok((send, recv))) => {
					// Get pairing handler
					let registry = protocol_registry.read().await;
					if let Some(handler) = registry.get_handler("pairing") {
						// Route to pairing protocol handler
						handler.handle_stream(
							Box::new(send),
							Box::new(recv),
							remote_node_id,
						).await;
					} else {
						eprintln!("Pairing protocol handler not registered");
					}
					// Continue to handle more streams on this connection
				}
				Ok(Err(e)) => {
					eprintln!("Failed to accept pairing stream: {}", e);
					break; // Exit loop on connection error
				}
				Err(_) => {
					// Timeout - connection is still active but no streams
					eprintln!("Timeout waiting for pairing stream from {}", remote_node_id);
					break; // Exit loop on timeout
				}
			}
		}
	}
	
	/// Handle a file transfer connection
	async fn handle_file_transfer_connection(
		conn: Connection,
		protocol_registry: Arc<RwLock<ProtocolRegistry>>,
		device_registry: Arc<RwLock<DeviceRegistry>>,
		event_sender: mpsc::UnboundedSender<NetworkEvent>,
		remote_node_id: NodeId,
	) {
		// Accept a unidirectional stream for file transfer
		match conn.accept_uni().await {
			Ok(recv) => {
				// Get file transfer handler
				let registry = protocol_registry.read().await;
				if let Some(handler) = registry.get_handler("file_transfer") {
					// Route to file transfer protocol handler
					handler.handle_stream(
						Box::new(tokio::io::empty()), // No send stream for uni-directional
						Box::new(recv),
						NodeId::from_bytes(&[0u8; 32]).unwrap(), // TODO: get actual node ID
					).await;
				} else {
					eprintln!("File transfer protocol handler not registered");
				}
			}
			Err(e) => {
				eprintln!("Failed to accept file transfer stream: {}", e);
			}
		}
	}
	
	/// Handle a messaging connection
	async fn handle_messaging_connection(
		conn: Connection,
		protocol_registry: Arc<RwLock<ProtocolRegistry>>,
		device_registry: Arc<RwLock<DeviceRegistry>>,
		event_sender: mpsc::UnboundedSender<NetworkEvent>,
		remote_node_id: NodeId,
	) {
		// Accept a bidirectional stream for messaging
		match conn.accept_bi().await {
			Ok((send, recv)) => {
				// Get messaging handler
				let registry = protocol_registry.read().await;
				if let Some(handler) = registry.get_handler("messaging") {
					// Route to messaging protocol handler
					handler.handle_stream(
						Box::new(send),
						Box::new(recv),
						remote_node_id,
					).await;
				} else {
					eprintln!("Messaging protocol handler not registered");
				}
			}
			Err(e) => {
				eprintln!("Failed to accept messaging stream: {}", e);
			}
		}
	}
	
	/// Handle a command from the main thread
	async fn handle_command(&self, command: EventLoopCommand) {
		match command {
			EventLoopCommand::ConnectionEstablished { device_id, node_id } => {
				// Update device registry
				let mut registry = self.device_registry.write().await;
				if let Err(e) = registry.set_device_connected(device_id, node_id) {
					eprintln!("Failed to update device connection state: {}", e);
				}
				
				// Send connection event
				let _ = self.event_sender.send(NetworkEvent::ConnectionEstablished {
					device_id,
					node_id,
				});
			}
			
			EventLoopCommand::SendMessage { device_id, protocol, data } => {
				// Look up node ID for device
				let node_id = {
					let registry = self.device_registry.read().await;
					registry.get_node_id_for_device(device_id)
				};
				
				if let Some(node_id) = node_id {
					// Send to node
					self.send_to_node(node_id, &protocol, data).await;
				} else {
					eprintln!("No node ID found for device {}", device_id);
				}
			}
			
			EventLoopCommand::SendMessageToNode { node_id, protocol, data } => {
				self.send_to_node(node_id, &protocol, data).await;
			}
			
			EventLoopCommand::Shutdown => {
				// Handled in main loop
			}
		}
	}
	
	/// Send a message to a specific node
	async fn send_to_node(&self, node_id: NodeId, protocol: &str, data: Vec<u8>) {
		println!("🚀 SEND_TO_NODE: Sending {} message to {} ({} bytes)", protocol, node_id, data.len());
		
		// Determine ALPN based on protocol
		let alpn = match protocol {
			"pairing" => PAIRING_ALPN,
			"file_transfer" => {
				println!("🔗 FILE_TRANSFER: Using ALPN: {:?}", String::from_utf8_lossy(FILE_TRANSFER_ALPN));
				FILE_TRANSFER_ALPN
			},
			"messaging" => MESSAGING_ALPN,
			_ => {
				eprintln!("Unknown protocol: {}", protocol);
				return;
			}
		};
		
		// Create node address (Iroh will use existing connection if available)
		let node_addr = NodeAddr::new(node_id);
		
		// Connect with specific ALPN
		println!("🔗 CONNECT: Attempting to connect to {} with ALPN: {:?}", node_id, String::from_utf8_lossy(alpn));
		match self.endpoint.connect(node_addr, alpn).await {
			Ok(conn) => {
				println!("✅ CONNECT: Successfully connected to {} with ALPN: {:?}", node_id, String::from_utf8_lossy(alpn));
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
									// Send message length first
									let len = data.len() as u32;
									if let Err(e) = send.write_all(&len.to_be_bytes()).await {
										eprintln!("Failed to write pairing message length: {}", e);
										return;
									}
								}
								
								// Send the data
								if let Err(e) = send.write_all(&data).await {
									eprintln!("Failed to send {} message: {}", protocol, e);
								}
								let _ = send.finish();
							}
							Err(e) => {
								eprintln!("Failed to open {} stream: {}", protocol, e);
							}
						}
					}
					"file_transfer" => {
						// Unidirectional stream
						println!("📤 FILE_TRANSFER: Opening unidirectional stream to {}", node_id);
						match conn.open_uni().await {
							Ok(mut send) => {
								println!("✅ FILE_TRANSFER: Opened stream, sending data");
								// Send with the expected format for file transfer protocol
								// Transfer type: 0 for file metadata request
								if let Err(e) = send.write_all(&[0u8]).await {
									eprintln!("Failed to write file transfer type: {}", e);
									return;
								}
								
								// Send message length (big-endian u32)
								let len = data.len() as u32;
								if let Err(e) = send.write_all(&len.to_be_bytes()).await {
									eprintln!("Failed to write file transfer message length: {}", e);
									return;
								}
								
								// Send the actual message data
								if let Err(e) = send.write_all(&data).await {
									eprintln!("Failed to send file transfer data: {}", e);
								} else {
									println!("📤 FILE_TRANSFER: Successfully sent {} bytes", data.len());
								}
								let _ = send.finish();
							}
							Err(e) => {
								eprintln!("❌ FILE_TRANSFER: Failed to open stream: {}", e);
							}
						}
					}
					_ => {}
				}
			}
			Err(e) => {
				eprintln!("Failed to connect to {}: {}", node_id, e);
			}
		}
	}
}