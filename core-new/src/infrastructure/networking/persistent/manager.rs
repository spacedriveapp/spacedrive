//! Persistent connection manager for auto-connecting to paired devices
//!
//! Manages the lifecycle of persistent connections, handling auto-reconnection,
//! retry logic, and overall connection orchestration for all paired devices.

use chrono::{DateTime, Duration, Utc};
use futures::StreamExt;
use libp2p::swarm::SwarmEvent;
use libp2p::{Multiaddr, PeerId, Swarm};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{interval, Interval};
use uuid::Uuid;

use super::{
	connection::{ConnectionEvent, ConnectionState, DeviceConnection},
	identity::{PersistentNetworkIdentity, SessionKeys, TrustLevel},
	messages::DeviceMessage,
};
use crate::device::DeviceManager;
use crate::networking::{
	DeviceInfo, EventSender, LibP2PEvent, NetworkError, NetworkIdentity, Result,
	SpacedriveBehaviour,
};

/// Configuration for the persistent connection manager
#[derive(Debug, Clone)]
pub struct ConnectionManagerConfig {
	/// Maximum number of concurrent connections
	pub max_connections: usize,
	/// Connection timeout in seconds
	pub connection_timeout_secs: u64,
	/// Retry interval for failed connections
	pub retry_interval_secs: u64,
	/// Maximum retry attempts before giving up
	pub max_retry_attempts: u32,
	/// Maintenance interval for connection health checks
	pub maintenance_interval_secs: u64,
	/// Keep-alive interval for connections
	pub keepalive_interval_secs: u64,
	/// Enable auto-reconnection
	pub auto_reconnect: bool,
}

/// Retry scheduler for failed connections
#[derive(Debug, Clone)]
pub struct RetryScheduler {
	/// Failed devices awaiting retry
	retry_queue: HashMap<Uuid, RetryInfo>,
	/// Next retry check time
	next_check: DateTime<Utc>,
}

/// Retry information for a failed device
#[derive(Debug, Clone)]
pub struct RetryInfo {
	/// Device ID
	pub device_id: Uuid,
	/// Number of attempts made
	pub attempts: u32,
	/// Next retry time
	pub next_retry: DateTime<Utc>,
	/// Last error message
	pub last_error: Option<String>,
	/// Backoff delay in seconds
	pub backoff_delay: u64,
}

/// Events emitted by the persistent connection manager
#[derive(Debug, Clone)]
pub enum NetworkEvent {
	/// Device connected and ready for communication
	DeviceConnected { device_id: Uuid },

	/// Device disconnected (network issue, shutdown, etc.)
	DeviceDisconnected { device_id: Uuid },

	/// Device trust was revoked
	DeviceRevoked { device_id: Uuid },

	/// New device pairing completed
	DevicePaired {
		device_id: Uuid,
		device_info: DeviceInfo,
	},

	/// Message received from a device
	MessageReceived {
		device_id: Uuid,
		message: DeviceMessage,
	},

	/// Connection error occurred
	ConnectionError {
		device_id: Option<Uuid>,
		error: NetworkError,
	},

	/// Connection attempt started
	ConnectionAttempt { device_id: Uuid, attempt: u32 },

	/// Retry scheduled for device
	RetryScheduled {
		device_id: Uuid,
		retry_at: DateTime<Utc>,
	},
}

/// Manages persistent connections to paired devices
pub struct PersistentConnectionManager {
	/// Local device identity
	local_identity: Arc<RwLock<PersistentNetworkIdentity>>,

	/// LibP2P swarm for network communication
	swarm: Swarm<SpacedriveBehaviour>,

	/// Active connections to devices
	active_connections: HashMap<Uuid, DeviceConnection>,

	/// Connection retry scheduler
	retry_scheduler: RetryScheduler,

	/// Event channels for core integration
	event_sender: EventSender,

	/// Connection event receiver
	connection_event_receiver: mpsc::UnboundedReceiver<ConnectionEvent>,

	/// Connection event sender (for device connections)
	connection_event_sender: mpsc::UnboundedSender<ConnectionEvent>,

	/// Configuration
	config: ConnectionManagerConfig,

	/// Maintenance timer
	maintenance_timer: Interval,

	/// Password for encrypted storage
	storage_password: String,

	/// Manager state
	is_running: bool,
}

impl Default for ConnectionManagerConfig {
	fn default() -> Self {
		Self {
			max_connections: 50,
			connection_timeout_secs: 30,
			retry_interval_secs: 60,
			max_retry_attempts: 5,
			maintenance_interval_secs: 30,
			keepalive_interval_secs: 30,
			auto_reconnect: true,
		}
	}
}

impl RetryScheduler {
	/// Create new retry scheduler
	pub fn new() -> Self {
		Self {
			retry_queue: HashMap::new(),
			next_check: Utc::now() + Duration::minutes(1),
		}
	}

	/// Schedule retry for a device
	pub fn schedule_retry(&mut self, device_id: Uuid, error: Option<String>) {
		let retry_info = self
			.retry_queue
			.entry(device_id)
			.or_insert_with(|| RetryInfo {
				device_id,
				attempts: 0,
				next_retry: Utc::now(),
				last_error: None,
				backoff_delay: 1,
			});

		retry_info.attempts += 1;
		retry_info.last_error = error;

		// Exponential backoff with jitter
		retry_info.backoff_delay = std::cmp::min(
			retry_info.backoff_delay * 2,
			300, // Max 5 minutes
		);

		// Add some jitter to prevent thundering herd
		let jitter = rand::random::<u64>() % (retry_info.backoff_delay / 4 + 1);
		retry_info.next_retry =
			Utc::now() + Duration::seconds((retry_info.backoff_delay + jitter) as i64);

		// Update next check time
		if retry_info.next_retry < self.next_check {
			self.next_check = retry_info.next_retry;
		}

		tracing::debug!(
			"Scheduled retry for device {} in {}s (attempt {})",
			device_id,
			retry_info.backoff_delay + jitter,
			retry_info.attempts
		);
	}

	/// Get devices ready for retry
	pub fn get_ready_retries(&mut self, max_attempts: u32) -> Vec<Uuid> {
		let now = Utc::now();
		let ready_devices: Vec<Uuid> = self
			.retry_queue
			.iter()
			.filter(|(_, info)| info.next_retry <= now && info.attempts < max_attempts)
			.map(|(&device_id, _)| device_id)
			.collect();

		// Update next check time
		self.next_check = self
			.retry_queue
			.values()
			.filter(|info| info.attempts < max_attempts)
			.map(|info| info.next_retry)
			.min()
			.unwrap_or_else(|| now + Duration::minutes(5));

		ready_devices
	}

	/// Remove device from retry queue (successful connection)
	pub fn remove_device(&mut self, device_id: &Uuid) {
		self.retry_queue.remove(device_id);
	}

	/// Get retry info for a device
	pub fn get_retry_info(&self, device_id: &Uuid) -> Option<&RetryInfo> {
		self.retry_queue.get(device_id)
	}
}

impl PersistentConnectionManager {
	/// Initialize with existing device identity
	pub async fn new(device_manager: &DeviceManager, password: &str) -> Result<Self> {
		Self::new_with_config(device_manager, password, ConnectionManagerConfig::default()).await
	}

	/// Initialize with custom configuration
	pub async fn new_with_config(
		device_manager: &DeviceManager,
		password: &str,
		config: ConnectionManagerConfig,
	) -> Result<Self> {
		// Load or create persistent network identity
		let identity = PersistentNetworkIdentity::load_or_create(device_manager, password).await?;
		let local_identity = Arc::new(RwLock::new(identity));

		// Initialize libp2p swarm with persistent identity
		let swarm = Self::create_swarm(&local_identity, password).await?;

		// Create event channels
		let (event_sender, _) = tokio::sync::mpsc::unbounded_channel();
		let (connection_event_sender, connection_event_receiver) =
			tokio::sync::mpsc::unbounded_channel();

		// Create maintenance timer
		let maintenance_timer = interval(std::time::Duration::from_secs(
			config.maintenance_interval_secs,
		));

		Ok(Self {
			local_identity,
			swarm,
			active_connections: HashMap::new(),
			retry_scheduler: RetryScheduler::new(),
			event_sender,
			connection_event_receiver,
			connection_event_sender,
			config,
			maintenance_timer,
			storage_password: password.to_string(),
			is_running: false,
		})
	}

	/// Create libp2p swarm from persistent identity
	async fn create_swarm(
		identity: &Arc<RwLock<PersistentNetworkIdentity>>,
		password: &str,
	) -> Result<Swarm<SpacedriveBehaviour>> {
		let identity_guard = identity.read().await;
		let network_identity = &identity_guard.identity;

		// Create a basic swarm structure for now
		// TODO: This needs proper integration with LibP2PManager
		use libp2p::{noise, tcp, yamux, SwarmBuilder};

		// Convert NetworkIdentity to libp2p identity (simplified approach)
		let local_keypair = Self::convert_identity_to_libp2p(network_identity, password)?;
		let local_peer_id = local_keypair.public().to_peer_id();

		let swarm = SwarmBuilder::with_existing_identity(local_keypair)
			.with_tokio()
			.with_tcp(
				tcp::Config::default(),
				noise::Config::new,
				yamux::Config::default,
			)
			.map_err(|e| NetworkError::TransportError(format!("Failed to configure TCP: {}", e)))?
			.with_quic()
			.with_behaviour(|_key| SpacedriveBehaviour::new(local_peer_id).unwrap())
			.map_err(|e| {
				NetworkError::TransportError(format!("Failed to create behaviour: {}", e))
			})?
			.with_swarm_config(|c| {
				c.with_idle_connection_timeout(std::time::Duration::from_secs(60))
			})
			.build();

		Ok(swarm)
	}

	/// Convert NetworkIdentity to libp2p Keypair (simplified version)
	fn convert_identity_to_libp2p(
		identity: &NetworkIdentity,
		password: &str,
	) -> Result<libp2p::identity::Keypair> {
		// Use deterministic keypair generation from device ID for consistency
		use blake3::Hasher;
		let mut hasher = Hasher::new();
		hasher.update(b"spacedrive-libp2p-keypair-v1");
		hasher.update(identity.device_id.as_bytes());
		hasher.update(identity.public_key.as_bytes());
		let seed = hasher.finalize();

		// Use first 32 bytes as Ed25519 seed
		let mut ed25519_seed = [0u8; 32];
		ed25519_seed.copy_from_slice(&seed.as_bytes()[..32]);

		let keypair = libp2p::identity::Keypair::ed25519_from_bytes(ed25519_seed).map_err(|e| {
			NetworkError::EncryptionError(format!("Failed to create Ed25519 keypair: {}", e))
		})?;

		Ok(keypair)
	}

	/// Start the connection manager
	pub async fn start(&mut self) -> Result<()> {
		if self.is_running {
			return Ok(());
		}

		self.is_running = true;
		tracing::info!("Starting persistent connection manager");

		// Start listening on configured transports
		self.start_listening().await?;

		// Start DHT discovery
		self.start_dht_discovery().await?;

		// Begin auto-connecting to paired devices
		self.start_auto_connections().await?;

		// Start the main event loop
		self.run_event_loop().await
	}

	/// Start listening on network transports
	async fn start_listening(&mut self) -> Result<()> {
		// Listen on TCP
		let tcp_addr: Multiaddr = "/ip4/0.0.0.0/tcp/0"
			.parse()
			.map_err(|e| NetworkError::TransportError(format!("Invalid TCP address: {}", e)))?;
		self.swarm
			.listen_on(tcp_addr)
			.map_err(|e| NetworkError::TransportError(format!("Failed to listen on TCP: {}", e)))?;

		// Listen on QUIC
		let quic_addr: Multiaddr = "/ip4/0.0.0.0/udp/0/quic-v1"
			.parse()
			.map_err(|e| NetworkError::TransportError(format!("Invalid QUIC address: {}", e)))?;
		self.swarm.listen_on(quic_addr).map_err(|e| {
			NetworkError::TransportError(format!("Failed to listen on QUIC: {}", e))
		})?;

		tracing::info!("Started listening on TCP and QUIC transports");
		Ok(())
	}

	/// Start DHT discovery
	async fn start_dht_discovery(&mut self) -> Result<()> {
		// Bootstrap DHT with known peers
		let bootstrap_peers: Vec<libp2p::Multiaddr> = vec![
			// Add bootstrap peer addresses here
		];

		for peer_addr in bootstrap_peers {
			if let Err(e) = self.swarm.dial(peer_addr.clone()) {
				tracing::debug!("Failed to dial bootstrap peer {}: {}", peer_addr, e);
			}
		}

		tracing::info!("Started DHT discovery");
		Ok(())
	}

	/// Start auto-connections to paired devices
	async fn start_auto_connections(&mut self) -> Result<()> {
		let auto_connect_devices = {
			let identity = self.local_identity.read().await;
			identity.auto_connect_devices()
		};

		tracing::info!(
			"Starting auto-connections to {} paired devices",
			auto_connect_devices.len()
		);

		for device_record in auto_connect_devices {
			let device_id = device_record.device_info.device_id;

			if self.active_connections.contains_key(&device_id) {
				continue; // Already connected
			}

			if let Err(e) = self.connect_to_device(device_id).await {
				tracing::warn!("Failed to auto-connect to device {}: {}", device_id, e);
				self.retry_scheduler
					.schedule_retry(device_id, Some(e.to_string()));
			}
		}

		Ok(())
	}

	/// Main event loop
	async fn run_event_loop(&mut self) -> Result<()> {
		tracing::info!("Starting connection manager event loop");

		loop {
			tokio::select! {
				// Handle swarm events
				Some(event) = self.swarm.next() => {
					self.handle_swarm_event(event).await;
				}

				// Handle connection events
				Some(event) = self.connection_event_receiver.recv() => {
					self.handle_connection_event(event).await;
				}

				// Perform maintenance
				_ = self.maintenance_timer.tick() => {
					self.perform_maintenance().await;
				}

				// Handle retry timer
				_ = tokio::time::sleep(std::time::Duration::from_secs(60)) => {
					self.handle_retries().await;
				}
			}
		}
	}

	/// Handle libp2p swarm events
	async fn handle_swarm_event(
		&mut self,
		event: SwarmEvent<super::super::behavior::SpacedriveBehaviourEvent>,
	) {
		match event {
			SwarmEvent::ConnectionEstablished { peer_id, .. } => {
				tracing::debug!("Connection established with peer: {}", peer_id);
				// Find which device this peer belongs to and mark as connected
				if let Some(device_id) = self.find_device_by_peer_id(&peer_id).await {
					self.on_device_connected(device_id).await;
				}
			}
			SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
				tracing::debug!("Connection closed with peer: {} - {:?}", peer_id, cause);
				if let Some(device_id) = self.find_device_by_peer_id(&peer_id).await {
					self.on_device_disconnected(device_id).await;
				}
			}
			SwarmEvent::NewListenAddr { address, .. } => {
				tracing::info!("Listening on: {}", address);
			}
			SwarmEvent::Behaviour(event) => {
				// Handle behavior-specific events
				self.handle_behaviour_event(event).await;
			}
			event => {
				tracing::debug!("Unhandled swarm event: {:?}", event);
			}
		}
	}

	/// Handle connection events from individual device connections
	async fn handle_connection_event(&mut self, event: ConnectionEvent) {
		match event {
			ConnectionEvent::StateChanged {
				device_id,
				new_state,
				..
			} => match new_state {
				ConnectionState::Connected => {
					self.retry_scheduler.remove_device(&device_id);
					if let Some(peer_id) = self.get_peer_id_for_device(&device_id) {
						let _ = self
							.event_sender
							.send(LibP2PEvent::ConnectionEstablished { peer_id });
					}
				}
				ConnectionState::Disconnected | ConnectionState::Failed(_) => {
					if self.config.auto_reconnect {
						self.retry_scheduler.schedule_retry(device_id, None);
					}
				}
				_ => {}
			},
			ConnectionEvent::MessageReceived { device_id, message } => {
				if let Some(peer_id) = self.get_peer_id_for_device(&device_id) {
					let _ = self.event_sender.send(LibP2PEvent::PairingResponse {
						peer_id,
						message: super::super::pairing::PairingMessage::PairingAccepted {
							timestamp: chrono::Utc::now(),
						},
					});
				}
			}
			ConnectionEvent::SendFailed {
				device_id, error, ..
			} => {
				tracing::error!("Failed to send message to device {}: {}", device_id, error);
			}
			ConnectionEvent::KeepaliveTimeout { device_id, .. } => {
				tracing::warn!("Keep-alive timeout for device {}", device_id);
				self.disconnect_from_device(device_id).await.ok();
			}
			ConnectionEvent::MetricsUpdated { device_id, metrics } => {
				tracing::debug!("Updated metrics for device {}: {:?}", device_id, metrics);
			}
		}
	}

	/// Handle behavior events from libp2p
	async fn handle_behaviour_event(
		&mut self,
		event: super::super::behavior::SpacedriveBehaviourEvent,
	) {
		use super::super::behavior::SpacedriveBehaviourEvent;
		
		match event {
			SpacedriveBehaviourEvent::Kademlia(kad_event) => {
				tracing::debug!("Kademlia event: {:?}", kad_event);
				// Forward to discovery handler if needed
			}
			SpacedriveBehaviourEvent::RequestResponse(req_resp_event) => {
				tracing::debug!("Request-response event: {:?}", req_resp_event);
				// Forward to pairing or other request handlers
			}
			SpacedriveBehaviourEvent::Mdns(mdns_event) => {
				tracing::debug!("mDNS event: {:?}", mdns_event);
				// Process mDNS discovery events for device discovery
				if let Some(discovered_peers) = self.extract_discovered_peers(&mdns_event) {
					for peer_id in discovered_peers {
						tracing::info!("Discovered peer via mDNS: {}", peer_id);
						// Could trigger connection attempts here
					}
				}
			}
		}
	}
	
	/// Extract discovered peers from mDNS events
	fn extract_discovered_peers(&self, mdns_event: &libp2p::mdns::Event) -> Option<Vec<libp2p::PeerId>> {
		match mdns_event {
			libp2p::mdns::Event::Discovered(list) => {
				Some(list.iter().map(|(peer_id, _)| *peer_id).collect())
			}
			libp2p::mdns::Event::Expired(list) => {
				tracing::debug!("mDNS peers expired: {:?}", list);
				None
			}
		}
	}

	/// Perform periodic maintenance
	async fn perform_maintenance(&mut self) {
		// Update connection metrics
		for connection in self.active_connections.values_mut() {
			connection.update_metrics();
		}

		// Check for maintenance needs
		let device_ids: Vec<Uuid> = self.active_connections.keys().cloned().collect();
		for device_id in device_ids {
			if let Some(connection) = self.active_connections.get_mut(&device_id) {
				let maintenance_actions = connection.needs_maintenance();

				for action in maintenance_actions {
					if let Err(e) = connection
						.perform_maintenance(action, &mut self.swarm)
						.await
					{
						tracing::error!("Maintenance failed for device {}: {}", device_id, e);
					}
				}

				// Process outbound message queue
				if let Err(e) = connection.process_outbound_queue(&mut self.swarm).await {
					tracing::error!(
						"Failed to process outbound queue for device {}: {}",
						device_id,
						e
					);
				}
			}
		}

		// Save identity if it has been updated
		let identity = self.local_identity.read().await;
		if let Err(e) = identity.save(&self.storage_password).await {
			tracing::error!("Failed to save persistent identity: {}", e);
		}
	}

	/// Handle connection retries
	async fn handle_retries(&mut self) {
		if !self.config.auto_reconnect {
			return;
		}

		let ready_devices = self
			.retry_scheduler
			.get_ready_retries(self.config.max_retry_attempts);

		for device_id in ready_devices {
			if self.active_connections.contains_key(&device_id) {
				self.retry_scheduler.remove_device(&device_id);
				continue;
			}

			if let Some(retry_info) = self.retry_scheduler.get_retry_info(&device_id) {
				if let Some(peer_id) = self.get_peer_id_for_device(&device_id) {
					let _ = self
						.event_sender
						.send(LibP2PEvent::ConnectionEstablished { peer_id });
				}

				tracing::info!(
					"Retrying connection to device {} (attempt {})",
					device_id,
					retry_info.attempts + 1
				);
			}

			match self.connect_to_device(device_id).await {
				Ok(()) => {
					self.retry_scheduler.remove_device(&device_id);
				}
				Err(e) => {
					tracing::warn!("Retry failed for device {}: {}", device_id, e);
					self.retry_scheduler
						.schedule_retry(device_id, Some(e.to_string()));
				}
			}
		}
	}

	/// Add a newly paired device
	pub async fn add_paired_device(
		&mut self,
		device_info: DeviceInfo,
		session_keys: SessionKeys,
	) -> Result<()> {
		let device_id = device_info.device_id;

		// Add to persistent identity
		{
			let mut identity = self.local_identity.write().await;
			identity.add_paired_device(
				device_info.clone(),
				session_keys,
				&self.storage_password,
			)?;
			identity.save(&self.storage_password).await?;
		}

		// Attempt immediate connection
		self.connect_to_device(device_id).await?;

		// Emit event
		if let Some(peer_id) = self.get_peer_id_for_device(&device_id) {
			let _ = self.event_sender.send(LibP2PEvent::DeviceDiscovered {
				peer_id,
				addr: "/ip4/127.0.0.1/tcp/0".parse().unwrap(), // Placeholder
			});
		}

		tracing::info!("Added paired device: {}", device_id);
		Ok(())
	}

	/// Connect to a specific device
	pub async fn connect_to_device(&mut self, device_id: Uuid) -> Result<()> {
		// Check if already connected
		if self.active_connections.contains_key(&device_id) {
			return Ok(());
		}

		// Check connection limit
		if self.active_connections.len() >= self.config.max_connections {
			return Err(NetworkError::ConnectionFailed(
				"Maximum connections reached".to_string(),
			));
		}

		let identity = self.local_identity.read().await;
		let device_record = identity
			.paired_devices
			.get(&device_id)
			.ok_or(NetworkError::DeviceNotFound(device_id))?
			.clone();

		// Skip if device is revoked
		if matches!(device_record.trust_level, TrustLevel::Revoked) {
			return Err(NetworkError::AuthenticationFailed(
				"Device trust revoked".to_string(),
			));
		}

		// Decrypt session keys
		let session_keys = if let Some(encrypted) = &device_record.session_keys {
			Some(identity.decrypt_session_keys(encrypted, &self.storage_password)?)
		} else {
			None
		};

		drop(identity); // Release read lock

		// Start connection process
		let connection = DeviceConnection::establish(
			&mut self.swarm,
			&device_record,
			session_keys,
			Some(self.connection_event_sender.clone()),
		)
		.await?;

		// Store active connection
		self.active_connections.insert(device_id, connection);

		// Update connection record
		{
			let mut identity = self.local_identity.write().await;
			identity.record_connection_success(&device_id, vec![]); // TODO: Get actual addresses
			identity.save(&self.storage_password).await?;
		}

		tracing::info!("Established connection to device: {}", device_id);
		Ok(())
	}

	/// Disconnect from a device
	pub async fn disconnect_from_device(&mut self, device_id: Uuid) -> Result<()> {
		if let Some(mut connection) = self.active_connections.remove(&device_id) {
			connection.close().await?;

			let _ = self.event_sender.send(LibP2PEvent::ConnectionClosed {
				peer_id: connection.peer_id(),
			});
		}
		Ok(())
	}

	/// Revoke trust for a device (removes pairing)
	pub async fn revoke_device(&mut self, device_id: Uuid) -> Result<()> {
		// Disconnect if currently connected
		self.disconnect_from_device(device_id).await?;

		// Mark as revoked in identity
		{
			let mut identity = self.local_identity.write().await;
			identity.update_trust_level(&device_id, TrustLevel::Revoked)?;
			identity.save(&self.storage_password).await?;
		}

		// Remove from retry queue
		self.retry_scheduler.remove_device(&device_id);

		tracing::info!("Revoked device: {}", device_id);
		Ok(())
	}

	/// Send message to a specific device
	pub async fn send_to_device(&mut self, device_id: Uuid, message: DeviceMessage) -> Result<()> {
		if let Some(connection) = self.active_connections.get_mut(&device_id) {
			connection.send_message(&mut self.swarm, message).await
		} else {
			Err(NetworkError::DeviceNotFound(device_id))
		}
	}

	/// Get all connected devices
	pub fn get_connected_devices(&self) -> Vec<Uuid> {
		self.active_connections
			.iter()
			.filter(|(_, conn)| matches!(conn.state(), ConnectionState::Connected))
			.map(|(&device_id, _)| device_id)
			.collect()
	}

	/// Get connection to a specific device
	pub fn get_connection(&self, device_id: &Uuid) -> Option<&DeviceConnection> {
		self.active_connections.get(device_id)
			.filter(|conn| matches!(conn.state(), ConnectionState::Connected))
	}

	/// Get the core network identity for pairing operations
	pub async fn get_network_identity(&self) -> Result<NetworkIdentity> {
		let identity = self.local_identity.read().await;
		Ok(identity.identity.clone())
	}

	/// Helper methods
	async fn find_device_by_peer_id(&self, peer_id: &PeerId) -> Option<Uuid> {
		for (device_id, connection) in &self.active_connections {
			if connection.peer_id() == *peer_id {
				return Some(*device_id);
			}
		}
		None
	}

	fn get_peer_id_for_device(&self, device_id: &Uuid) -> Option<PeerId> {
		self.active_connections
			.get(device_id)
			.map(|conn| conn.peer_id())
	}

	async fn on_device_connected(&mut self, device_id: Uuid) {
		tracing::info!("Device connected: {}", device_id);
		// Update connection state and emit events
	}

	async fn on_device_disconnected(&mut self, device_id: Uuid) {
		tracing::info!("Device disconnected: {}", device_id);

		// Update identity
		{
			let mut identity = self.local_identity.write().await;
			identity.record_connection_failure(&device_id);
			if let Err(e) = identity.save(&self.storage_password).await {
				tracing::error!("Failed to save identity after disconnection: {}", e);
			}
		}

		// Schedule retry if auto-reconnect is enabled
		if self.config.auto_reconnect {
			self.retry_scheduler.schedule_retry(device_id, None);
		}
	}
}
