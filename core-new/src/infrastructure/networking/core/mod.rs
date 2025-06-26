//! Core networking engine with unified LibP2P swarm

pub mod behavior;
pub mod discovery;
pub mod event_loop;
pub mod swarm;

use crate::device::DeviceManager;
use crate::infrastructure::networking::{
	device::{DeviceInfo, DeviceRegistry},
	protocols::{pairing::PairingProtocolHandler, ProtocolRegistry},
	utils::{logging::ConsoleLogger, NetworkIdentity},
	NetworkingError, Result,
};
use libp2p::{kad::{QueryId, RecordKey}, Multiaddr, PeerId, Swarm};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

pub use behavior::{UnifiedBehaviour, UnifiedBehaviourEvent};
pub use event_loop::NetworkingEventLoop;

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

/// Main networking core - single source of truth for all networking operations
pub struct NetworkingCore {
	/// Our network identity
	identity: NetworkIdentity,

	/// LibP2P swarm with unified behavior
	swarm: Swarm<UnifiedBehaviour>,

	/// Shutdown sender for stopping the event loop
	shutdown_sender: Option<mpsc::UnboundedSender<()>>,

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

impl NetworkingCore {
	/// Create a new networking core
	pub async fn new(device_manager: Arc<DeviceManager>) -> Result<Self> {
		// Generate network identity
		let identity = NetworkIdentity::new().await?;

		// Create LibP2P swarm
		let swarm = swarm::create_swarm(identity.clone()).await?;

		// Create event channel
		let (event_sender, event_receiver) = mpsc::unbounded_channel();

		// Create registries
		let protocol_registry = Arc::new(RwLock::new(ProtocolRegistry::new()));
		let device_registry = Arc::new(RwLock::new(DeviceRegistry::new(device_manager)));
		
		// Note: Protocol handlers will be registered by the Core during init_networking
		// to avoid duplicate registrations

		Ok(Self {
			identity,
			swarm,
			shutdown_sender: None,
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
		self.shutdown_sender = Some(shutdown_sender);
		self.command_sender = Some(command_sender);

		Ok(())
	}

	/// Stop the networking service
	pub async fn shutdown(&mut self) -> Result<()> {
		if let Some(shutdown_sender) = self.shutdown_sender.take() {
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
	pub async fn send_message_to_peer(&self, peer_id: PeerId, protocol: &str, data: Vec<u8>) -> Result<()> {
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
								println!("No external addresses found on attempt {}, retrying...", attempt);
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
	
}

// Ensure NetworkingCore is Send + Sync for proper async usage
unsafe impl Send for NetworkingCore {}
unsafe impl Sync for NetworkingCore {}
