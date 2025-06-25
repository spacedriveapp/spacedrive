//! Central event loop for processing LibP2P events

use super::{
	behavior::{UnifiedBehaviour, UnifiedBehaviourEvent},
	NetworkEvent,
};
use crate::infrastructure::networking::{
	device::{DeviceConnection, DeviceInfo, DeviceRegistry},
	protocols::{ProtocolEvent, ProtocolRegistry},
	utils::NetworkIdentity,
	NetworkingError, Result,
};
use futures::StreamExt;
use libp2p::{
	kad::{self, QueryId, RecordKey},
	swarm::{Swarm, SwarmEvent},
	Multiaddr, PeerId,
};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

/// Commands that can be sent to the event loop
#[derive(Debug)]
pub enum EventLoopCommand {
	SendMessage {
		device_id: Uuid,
		protocol: String,
		data: Vec<u8>,
	},
	SendMessageToPeer {
		peer_id: PeerId,
		protocol: String,
		data: Vec<u8>,
	},
	/// Publish a DHT record for pairing session discovery
	PublishDhtRecord {
		key: RecordKey,
		value: Vec<u8>,
		response_channel: tokio::sync::oneshot::Sender<Result<QueryId>>,
	},
	/// Query a DHT record for pairing session discovery
	QueryDhtRecord {
		key: RecordKey,
		response_channel: tokio::sync::oneshot::Sender<Result<QueryId>>,
	},
	/// Get current listening addresses from the swarm
	GetListeningAddresses {
		response_channel: tokio::sync::oneshot::Sender<Vec<Multiaddr>>,
	},
}

/// Central event loop for processing all LibP2P events
pub struct NetworkingEventLoop {
	/// LibP2P swarm
	swarm: Swarm<UnifiedBehaviour>,

	/// Protocol registry for handling messages
	protocol_registry: Arc<RwLock<ProtocolRegistry>>,

	/// Device registry for state management
	device_registry: Arc<RwLock<DeviceRegistry>>,

	/// Event sender for broadcasting events
	event_sender: mpsc::UnboundedSender<NetworkEvent>,

	/// Network identity for signing and key operations
	identity: NetworkIdentity,

	/// Channel for receiving shutdown signal
	shutdown_receiver: Option<mpsc::UnboundedReceiver<()>>,

	/// Channel for sending shutdown signal
	shutdown_sender: mpsc::UnboundedSender<()>,

	/// Channel for receiving commands
	command_receiver: Option<mpsc::UnboundedReceiver<EventLoopCommand>>,

	/// Channel for sending commands
	command_sender: mpsc::UnboundedSender<EventLoopCommand>,

	/// Running state
	is_running: bool,

	/// Pending pairing sessions where we're waiting for connections
	/// Maps session_id -> (peer_id, device_info, retry_count, last_attempt)
	pending_pairing_connections: std::collections::HashMap<
		Uuid,
		(
			PeerId,
			crate::infrastructure::networking::device::DeviceInfo,
			u32,
			chrono::DateTime<chrono::Utc>,
		),
	>,
}

impl NetworkingEventLoop {
	/// Create a new event loop
	pub fn new(
		swarm: Swarm<UnifiedBehaviour>,
		protocol_registry: Arc<RwLock<ProtocolRegistry>>,
		device_registry: Arc<RwLock<DeviceRegistry>>,
		event_sender: mpsc::UnboundedSender<NetworkEvent>,
		identity: NetworkIdentity,
	) -> Self {
		let (shutdown_sender, shutdown_receiver) = mpsc::unbounded_channel();
		let (command_sender, command_receiver) = mpsc::unbounded_channel();

		Self {
			swarm,
			protocol_registry,
			device_registry,
			event_sender,
			identity,
			shutdown_receiver: Some(shutdown_receiver),
			shutdown_sender,
			command_receiver: Some(command_receiver),
			command_sender,
			is_running: false,
			pending_pairing_connections: HashMap::new(),
		}
	}

	/// Start the event loop (consumes self to move swarm ownership)
	pub async fn start(mut self) -> Result<()> {
		if self.is_running {
			return Err(NetworkingError::Protocol(
				"Event loop already running".to_string(),
			));
		}

		// Take shutdown receiver
		let mut shutdown_receiver = self
			.shutdown_receiver
			.take()
			.ok_or_else(|| NetworkingError::Protocol("Event loop already started".to_string()))?;

		// Take command receiver
		let mut command_receiver = self
			.command_receiver
			.take()
			.ok_or_else(|| NetworkingError::Protocol("Event loop already started".to_string()))?;

		// Clone necessary components for the task
		let protocol_registry = self.protocol_registry.clone();
		let device_registry = self.device_registry.clone();
		let event_sender = self.event_sender.clone();

		// Move swarm, state, and identity into the task
		let mut swarm = self.swarm;
		let mut pending_pairing_connections = self.pending_pairing_connections;
		let identity = self.identity;

		// Spawn the main event processing task
		tokio::spawn(async move {
			loop {
				tokio::select! {
					// Handle shutdown signal
					_ = shutdown_receiver.recv() => {
						println!("Event loop shutdown requested");
						break;
					}

					// Handle commands
					Some(command) = command_receiver.recv() => {
						if let Err(e) = Self::handle_command(
							command,
							&mut swarm,
							&device_registry,
						).await {
							eprintln!("Error handling command: {}", e);
						}
					}

					// Handle LibP2P swarm events
					event = swarm.select_next_some() => {
						if let Err(e) = Self::handle_swarm_event(
							event,
							&protocol_registry,
							&device_registry,
							&mut swarm,
							&mut pending_pairing_connections,
							&identity,
							&event_sender,
						).await {
							eprintln!("Error handling swarm event: {}", e);
						}
					}
				}
			}

			println!("Event loop stopped");
		});

		Ok(())
	}

	/// Get a shutdown sender for stopping the event loop
	pub fn shutdown_sender(&self) -> mpsc::UnboundedSender<()> {
		self.shutdown_sender.clone()
	}

	/// Get a command sender for sending commands to the event loop
	pub fn command_sender(&self) -> mpsc::UnboundedSender<EventLoopCommand> {
		self.command_sender.clone()
	}

	/// Send a message to a device via the command channel
	pub async fn send_message(&self, device_id: Uuid, protocol: &str, data: Vec<u8>) -> Result<()> {
		let command = EventLoopCommand::SendMessage {
			device_id,
			protocol: protocol.to_string(),
			data,
		};

		self.command_sender
			.send(command)
			.map_err(|_| NetworkingError::ConnectionFailed("Event loop not running".to_string()))?;

		Ok(())
	}

	/// Handle commands sent to the event loop
	async fn handle_command(
		command: EventLoopCommand,
		swarm: &mut Swarm<UnifiedBehaviour>,
		device_registry: &Arc<RwLock<DeviceRegistry>>,
	) -> Result<()> {
		match command {
			EventLoopCommand::SendMessage {
				device_id,
				protocol,
				data,
			} => {
				// Look up the peer ID for the device
				if let Some(peer_id) = device_registry.read().await.get_peer_by_device(device_id) {
					println!(
						"Sending {} message to device {} (peer {}): {} bytes",
						protocol,
						device_id,
						peer_id,
						data.len()
					);

					// Send the message via the appropriate protocol
					match protocol.as_str() {
						"pairing" => {
							// Send pairing message
							if let Ok(message) =
								serde_json::from_slice::<super::behavior::PairingMessage>(&data)
							{
								let request_id = swarm
									.behaviour_mut()
									.pairing
									.send_request(&peer_id, message);
								println!("Sent pairing request with ID: {:?}", request_id);
							}
						}
						"messaging" => {
							// Send generic message
							if let Ok(message) =
								serde_json::from_slice::<super::behavior::DeviceMessage>(&data)
							{
								let request_id = swarm
									.behaviour_mut()
									.messaging
									.send_request(&peer_id, message);
								println!("Sent message request with ID: {:?}", request_id);
							}
						}
						"file_transfer" => {
							// Send file transfer message
							use crate::infrastructure::networking::protocols::file_transfer::FileTransferMessage;
							if let Ok(message) = rmp_serde::from_slice::<FileTransferMessage>(&data)
							{
								let request_id = swarm
									.behaviour_mut()
									.file_transfer
									.send_request(&peer_id, message);
								println!("📤 Sent file transfer request with ID: {:?}", request_id);
							} else {
								println!("❌ Failed to deserialize file transfer message");
							}
						}
						_ => {
							println!("Unknown protocol: {}", protocol);
						}
					}
				} else {
					println!("Device {} not found or not connected", device_id);
				}
			}
			EventLoopCommand::SendMessageToPeer {
				peer_id,
				protocol,
				data,
			} => {
				println!(
					"Sending {} message to peer {}: {} bytes",
					protocol,
					peer_id,
					data.len()
				);

				// Send the message directly via the appropriate protocol
				match protocol.as_str() {
					"pairing" => {
						// Send pairing message
						if let Ok(message) =
							serde_json::from_slice::<super::behavior::PairingMessage>(&data)
						{
							let request_id = swarm
								.behaviour_mut()
								.pairing
								.send_request(&peer_id, message);
							println!("Sent direct pairing request with ID: {:?}", request_id);
						}
					}
					"messaging" => {
						// Send generic message
						if let Ok(message) =
							serde_json::from_slice::<super::behavior::DeviceMessage>(&data)
						{
							let request_id = swarm
								.behaviour_mut()
								.messaging
								.send_request(&peer_id, message);
							println!("Sent direct message request with ID: {:?}", request_id);
						}
					}
					_ => {
						println!("Unknown protocol: {}", protocol);
					}
				}
			}
			EventLoopCommand::PublishDhtRecord {
				key,
				value,
				response_channel,
			} => {
				// Create a DHT record with the provided key and value
				let record = kad::Record::new(key, value);

				// Publish the record to the DHT
				match swarm
					.behaviour_mut()
					.kademlia
					.put_record(record, kad::Quorum::One)
				{
					Ok(query_id) => {
						println!("Publishing DHT record with query ID: {:?}", query_id);
						let _ = response_channel.send(Ok(query_id));
					}
					Err(e) => {
						println!("Failed to publish DHT record: {:?}", e);
						let _ = response_channel.send(Err(NetworkingError::Protocol(format!(
							"DHT put failed: {:?}",
							e
						))));
					}
				}
			}
			EventLoopCommand::QueryDhtRecord {
				key,
				response_channel,
			} => {
				// Query the DHT for the record
				let query_id = swarm.behaviour_mut().kademlia.get_record(key);

				println!("Querying DHT record with query ID: {:?}", query_id);

				// Send the query ID back to the caller
				let _ = response_channel.send(Ok(query_id));
			}
			EventLoopCommand::GetListeningAddresses { response_channel } => {
				// Get all current listening addresses from the swarm
				let addresses: Vec<Multiaddr> = swarm.listeners().cloned().collect();

				// Filter out invalid or non-routable addresses
				let external_addresses: Vec<Multiaddr> = addresses
					.into_iter()
					.filter(|addr| {
						// Remove localhost and zero port addresses
						let addr_str = addr.to_string();
						!addr_str.contains("127.0.0.1")
							&& !addr_str.contains("tcp/0")
							&& !addr_str.contains("::1")
					})
					.collect();

				println!("Current listening addresses: {:?}", external_addresses);

				// Send the addresses back to the caller
				let _ = response_channel.send(external_addresses);
			}
		}

		Ok(())
	}

	/// Handle a single swarm event
	async fn handle_swarm_event(
		event: SwarmEvent<UnifiedBehaviourEvent>,
		protocol_registry: &Arc<RwLock<ProtocolRegistry>>,
		device_registry: &Arc<RwLock<DeviceRegistry>>,
		swarm: &mut Swarm<UnifiedBehaviour>,
		pending_pairing_connections: &mut HashMap<
			Uuid,
			(
				PeerId,
				crate::infrastructure::networking::device::DeviceInfo,
				u32,
				chrono::DateTime<chrono::Utc>,
			),
		>,
		identity: &NetworkIdentity,
		event_sender: &mpsc::UnboundedSender<NetworkEvent>,
	) -> Result<()> {
		match event {
			SwarmEvent::NewListenAddr { address, .. } => {
				println!("Listening on: {}", address);
			}

			SwarmEvent::ConnectionEstablished {
				peer_id, endpoint, ..
			} => {
				println!(
					"Connection established with: {} at {}",
					peer_id,
					endpoint.get_remote_address()
				);

				// CRITICAL FIX: Ensure connected peer is in Kademlia routing table
				// This is needed when connections are established without prior mDNS discovery
				swarm
					.behaviour_mut()
					.kademlia
					.add_address(&peer_id, endpoint.get_remote_address().clone());
				println!("Added connected peer {} to Kademlia routing table", peer_id);

				// Check if this is a pending pairing connection
				let pending_session = pending_pairing_connections
					.iter()
					.find(|(_, (pending_peer_id, _, _, _))| *pending_peer_id == peer_id)
					.map(|(session_id, (_, device_info, _, _))| (*session_id, device_info.clone()));

				if let Some((session_id, device_info)) = pending_session {
					println!(
						"Connection established for pairing session: {} with peer: {}",
						session_id, peer_id
					);

					// Send pairing request message
					let pairing_request = crate::infrastructure::networking::protocols::pairing::PairingMessage::PairingRequest {
						session_id,
						device_info: device_info.clone(),
						public_key: identity.public_key_bytes(),
					};

					let request_id = swarm
						.behaviour_mut()
						.pairing
						.send_request(&peer_id, pairing_request);

					println!(
						"Sent pairing request for session {} with request ID: {:?}",
						session_id, request_id
					);

					// Remove from pending connections
					pending_pairing_connections
						.retain(|_, (pending_peer_id, _, _, _)| *pending_peer_id != peer_id);
				} else {
					// Normal connection - look up device by peer ID
					if let Some(device_id) =
						device_registry.read().await.get_device_by_peer(peer_id)
					{
						let _ = event_sender
							.send(NetworkEvent::ConnectionEstablished { device_id, peer_id });
					}
				}
			}

			SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
				println!("Connection closed with {}: {:?}", peer_id, cause);

				// Check if this was a pending pairing connection that failed
				let failed_pairing = pending_pairing_connections
					.iter()
					.find(|(_, (pending_peer_id, _, _, _))| *pending_peer_id == peer_id)
					.map(|(session_id, (_, device_info, retry_count, _))| {
						(*session_id, device_info.clone(), *retry_count)
					});

				if let Some((session_id, device_info, retry_count)) = failed_pairing {
					const MAX_RETRIES: u32 = 3;

					if retry_count < MAX_RETRIES {
						println!(
							"Pairing connection failed for session {}, retrying ({}/{})",
							session_id,
							retry_count + 1,
							MAX_RETRIES
						);

						// Update retry count and last attempt time
						if let Some((_, _, ref mut count, ref mut last_attempt)) =
							pending_pairing_connections.get_mut(&session_id)
						{
							*count += 1;
							*last_attempt = chrono::Utc::now();
						}

						// TODO: Implement proper retry mechanism through command channel
						println!("Scheduling retry for pairing session {}", session_id);
					} else {
						println!(
							"Pairing connection failed permanently for session {} after {} retries",
							session_id, MAX_RETRIES
						);

						// Remove from pending connections and emit failure event
						pending_pairing_connections.remove(&session_id);

						let _ = event_sender.send(NetworkEvent::PairingFailed {
							session_id,
							reason: format!(
								"Connection failed after {} retries: {:?}",
								MAX_RETRIES, cause
							),
						});
					}
				} else {
					// Normal disconnection - look up device by peer ID
					if let Some(device_id) =
						device_registry.read().await.get_device_by_peer(peer_id)
					{
						let _ = device_registry.write().await.mark_disconnected(
							device_id,
							crate::infrastructure::networking::device::DisconnectionReason::NetworkError(
								format!("{:?}", cause)
							),
						);

						let _ =
							event_sender.send(NetworkEvent::ConnectionLost { device_id, peer_id });
					}
				}
			}

			SwarmEvent::Behaviour(behaviour_event) => {
				Self::handle_behaviour_event(
					behaviour_event,
					protocol_registry,
					device_registry,
					swarm,
					pending_pairing_connections,
					identity,
					event_sender,
				)
				.await?;
			}

			_ => {
				// Handle other events as needed
			}
		}

		Ok(())
	}

	/// Schedule pairing requests for mDNS discovered peers (wait for connection establishment)
	/// Returns the number of pairing sessions scheduled for connection
	async fn schedule_pairing_on_mdns_discovery(
		protocol_registry: &Arc<RwLock<ProtocolRegistry>>,
		identity: &NetworkIdentity,
		discovered_peer_id: PeerId,
		pending_pairing_connections: &mut std::collections::HashMap<
			uuid::Uuid,
			(
				PeerId,
				crate::infrastructure::networking::device::DeviceInfo,
				u32,
				chrono::DateTime<chrono::Utc>,
			),
		>,
	) -> Result<u32> {
		let mut sessions_scheduled = 0;

		// Get pairing handler from protocol registry with proper error handling
		let registry = protocol_registry.read().await;
		let pairing_handler = match registry.get_handler("pairing") {
			Some(handler) => handler,
			None => {
				// No pairing handler registered - this is normal if pairing is not active
				return Ok(0);
			}
		};

		// Downcast to concrete pairing handler type
		let pairing_handler =
			match pairing_handler
				.as_any()
				.downcast_ref::<crate::infrastructure::networking::protocols::pairing::PairingProtocolHandler>(
			) {
				Some(handler) => handler,
				None => {
					return Err(NetworkingError::Protocol(
						"Invalid pairing handler type".to_string(),
					));
				}
			};

		// Get active pairing sessions
		let active_sessions = pairing_handler.get_active_sessions().await;

		// Process each session that's actively scanning for peers
		for session in &active_sessions {
			// Only schedule requests for sessions where we're actively scanning (Bob's role)
			if matches!(
				session.state,
				crate::infrastructure::networking::protocols::pairing::PairingState::Scanning
			) {
				println!("🔍 Found scanning session {} - scheduling pairing request for peer {} (waiting for connection)", session.id, discovered_peer_id);

				// Create device info for this session
				let device_info = crate::infrastructure::networking::device::DeviceInfo {
					device_id: identity.device_id(),
					device_name: Self::get_device_name_for_pairing(),
					device_type: crate::infrastructure::networking::device::DeviceType::Desktop,
					os_version: std::env::consts::OS.to_string(),
					app_version: env!("CARGO_PKG_VERSION").to_string(),
					network_fingerprint: identity.network_fingerprint(),
					last_seen: chrono::Utc::now(),
				};

				// Add to pending connections - pairing request will be sent after connection establishment
				// Using 5-minute timeout (300 seconds) and current time
				pending_pairing_connections.insert(
					session.id,
					(discovered_peer_id, device_info, 300, chrono::Utc::now()),
				);

				println!("✅ mDNS Discovery: Scheduled pairing request for session {} with peer {} (pending connection)",
						 session.id, discovered_peer_id);

				sessions_scheduled += 1;
			} else {
				// Log other session states for debugging
				match &session.state {
					crate::infrastructure::networking::protocols::pairing::PairingState::WaitingForConnection => {
						// This is Alice waiting for Bob - don't schedule requests
						println!("🔍 Found waiting session {} (Alice side) - not scheduling request", session.id);
					}
					_ => {
						// Other states like Completed, Failed, etc.
						println!("🔍 Found session {} in state {} - not scheduling request", session.id, session.state);
					}
				}
			}
		}

		if sessions_scheduled == 0 && !active_sessions.is_empty() {
			println!(
				"🔍 mDNS Discovery: Found {} active sessions but none in Scanning state",
				active_sessions.len()
			);
		}

		Ok(sessions_scheduled)
	}

	/// Get device name for pairing (production-ready with fallback)
	fn get_device_name_for_pairing() -> String {
		// Try to get hostname first
		if let Ok(hostname) = std::env::var("HOSTNAME") {
			if !hostname.is_empty() {
				return format!("{} (Spacedrive)", hostname);
			}
		}

		// Fallback to OS-specific naming
		match std::env::consts::OS {
			"macos" => "Mac (Spacedrive)".to_string(),
			"windows" => "Windows PC (Spacedrive)".to_string(),
			"linux" => "Linux (Spacedrive)".to_string(),
			_ => "Spacedrive Device".to_string(),
		}
	}

	/// Handle behavior-specific events
	async fn handle_behaviour_event(
		event: UnifiedBehaviourEvent,
		protocol_registry: &Arc<RwLock<ProtocolRegistry>>,
		device_registry: &Arc<RwLock<DeviceRegistry>>,
		swarm: &mut Swarm<UnifiedBehaviour>,
		pending_pairing_connections: &mut HashMap<
			Uuid,
			(
				PeerId,
				crate::infrastructure::networking::device::DeviceInfo,
				u32,
				chrono::DateTime<chrono::Utc>,
			),
		>,
		identity: &NetworkIdentity,
		event_sender: &mpsc::UnboundedSender<NetworkEvent>,
	) -> Result<()> {
		match event {
			UnifiedBehaviourEvent::Kademlia(kad_event) => {
				use libp2p::kad;
				match kad_event {
					kad::Event::OutboundQueryProgressed {
						id,
						result: kad::QueryResult::PutRecord(put_result),
						..
					} => match put_result {
						Ok(kad::PutRecordOk { key }) => {
							println!(
								"DHT record published successfully: query_id={:?}, key={:?}",
								id, key
							);
						}
						Err(kad::PutRecordError::QuorumFailed {
							key,
							success,
							quorum,
						}) => {
							println!("DHT record publish failed: query_id={:?}, key={:?}, success={:?}, quorum={:?}", id, key, success, quorum);
						}
						Err(kad::PutRecordError::Timeout { key, .. }) => {
							println!(
								"DHT record publish timed out: query_id={:?}, key={:?}",
								id, key
							);
						}
					},
					kad::Event::OutboundQueryProgressed {
						id,
						result: kad::QueryResult::GetRecord(get_result),
						..
					} => {
						match get_result {
							Ok(kad::GetRecordOk::FoundRecord(record)) => {
								println!(
									"DHT record found: query_id={:?}, key={:?}, {} bytes",
									id,
									record.record.key,
									record.record.value.len()
								);

								// Try to deserialize as pairing advertisement
								if let Ok(advertisement) = serde_json::from_slice::<crate::infrastructure::networking::protocols::pairing::PairingAdvertisement>(&record.record.value) {
									println!("Found pairing advertisement from peer: {:?}", advertisement.peer_id);

									// Convert strings back to libp2p types
									if let (Ok(peer_id), Ok(addresses)) = (advertisement.peer_id(), advertisement.addresses()) {
										// Extract session ID from the DHT key
										if let Ok(session_id_bytes) = record.record.key.as_ref().try_into() {
											let session_id = Uuid::from_bytes(session_id_bytes);

											// Emit pairing discovery event
											let _ = event_sender.send(NetworkEvent::PairingSessionDiscovered {
												session_id,
												peer_id,
												addresses: addresses.clone(),
												device_info: advertisement.device_info.clone(),
											});

											println!("Emitted pairing session discovery event for session: {}", session_id);

											// Automatically connect to the discovered peer
											for address in &addresses {
												match swarm.dial(address.clone()) {
													Ok(_) => {
														println!("Dialing discovered peer {} at {}", peer_id, address);

														// Track this as a pending pairing connection with retry info
														pending_pairing_connections.insert(session_id, (peer_id, advertisement.device_info.clone(), 0, chrono::Utc::now()));
														println!("Tracking pending pairing connection for session: {} -> peer: {}", session_id, peer_id);

														break; // Try only the first successful dial
													}
													Err(e) => {
														println!("Failed to dial {}: {:?}", address, e);
													}
												}
											}
										}
									} else {
										println!("Failed to parse peer_id or addresses from pairing advertisement");
									}
								}
							}
							Ok(kad::GetRecordOk::FinishedWithNoAdditionalRecord { .. }) => {
								println!(
									"DHT query finished, no additional records: query_id={:?}",
									id
								);
							}
							Err(kad::GetRecordError::NotFound { key, .. }) => {
								println!("DHT record not found: query_id={:?}, key={:?}", id, key);

								// Emit pairing failure event for not found records
								if let Ok(session_id_bytes) = key.as_ref().try_into() {
									let session_id = Uuid::from_bytes(session_id_bytes);
									let _ = event_sender.send(NetworkEvent::PairingFailed {
										session_id,
										reason: "Pairing session not found in DHT".to_string(),
									});
								}
							}
							Err(kad::GetRecordError::QuorumFailed { key, .. }) => {
								println!(
									"DHT query quorum failed: query_id={:?}, key={:?}",
									id, key
								);

								// For quorum failures, we could implement retry logic
								println!("DHT quorum failed - network may be degraded");
							}
							Err(kad::GetRecordError::Timeout { key }) => {
								println!("DHT query timed out: query_id={:?}, key={:?}", id, key);

								// For timeouts, emit failure event as the session likely expired
								if let Ok(session_id_bytes) = key.as_ref().try_into() {
									let session_id = Uuid::from_bytes(session_id_bytes);
									let _ = event_sender.send(NetworkEvent::PairingFailed {
										session_id,
										reason: "DHT query timed out - session may have expired"
											.to_string(),
									});
								}
							}
						}
					}
					kad::Event::ModeChanged { new_mode } => {
						println!("Kademlia mode changed: {:?}", new_mode);
					}
					kad::Event::RoutingUpdated { peer, .. } => {
						println!("Kademlia routing updated: peer={}", peer);
					}
					kad::Event::UnroutablePeer { peer } => {
						println!("Kademlia unroutable peer: {}", peer);
					}
					kad::Event::RoutablePeer { peer, .. } => {
						println!("Kademlia routable peer: {}", peer);
					}
					kad::Event::PendingRoutablePeer { peer, .. } => {
						println!("Kademlia pending routable peer: {}", peer);
					}
					_ => {
						println!("Other Kademlia event: {:?}", kad_event);
					}
				}
			}

			UnifiedBehaviourEvent::Mdns(mdns_event) => {
				use libp2p::mdns;
				match mdns_event {
					mdns::Event::Discovered(list) => {
						for (peer_id, addr) in list {
							println!("Discovered peer via mDNS: {} at {}", peer_id, addr);

							// CRITICAL FIX: Add discovered peer to Kademlia DHT routing table
							// This enables DHT operations between locally discovered peers
							swarm
								.behaviour_mut()
								.kademlia
								.add_address(&peer_id, addr.clone());
							println!(
								"Added peer {} to Kademlia routing table with address {}",
								peer_id, addr
							);

							// Bootstrap the Kademlia DHT if this is our first peer
							// This activates the DHT network between discovered peers
							if let Ok(query_id) = swarm.behaviour_mut().kademlia.bootstrap() {
								println!(
									"Bootstrapping Kademlia DHT with query ID: {:?}",
									query_id
								);
							}

							// PRODUCTION: Schedule pairing requests for mDNS discovered peers (wait for connection)
							// This handles the case where Bob discovers Alice via mDNS during pairing
							match Self::schedule_pairing_on_mdns_discovery(
								&protocol_registry,
								&identity,
								peer_id,
								pending_pairing_connections,
							)
							.await
							{
								Ok(sessions_scheduled) => {
									if sessions_scheduled > 0 {
										println!("🔍 mDNS Discovery: Scheduled {} pairing sessions for peer {} (waiting for connection)", sessions_scheduled, peer_id);
									}
								}
								Err(e) => {
									println!("⚠️ mDNS Discovery: Failed to schedule pairing with peer {}: {}", peer_id, e);
								}
							}

							let _ = event_sender.send(NetworkEvent::PeerDiscovered {
								peer_id,
								addresses: vec![addr],
							});
						}
					}
					mdns::Event::Expired(list) => {
						for (peer_id, _) in list {
							println!("mDNS peer expired: {}", peer_id);

							let _ = event_sender.send(NetworkEvent::PeerDisconnected { peer_id });
						}
					}
				}
			}

			UnifiedBehaviourEvent::Pairing(req_resp_event) => {
				use libp2p::request_response;
				match req_resp_event {
					request_response::Event::Message {
						peer,
						message,
						connection_id: _,
					} => match message {
						request_response::Message::Request {
							request,
							channel,
							request_id,
						} => {
							println!("Received pairing request from {}", peer);

							// Extract session_id and device_id from the pairing message
							let (session_id, device_id_from_request) = match &request {
								super::behavior::PairingMessage::PairingRequest {
									session_id,
									device_info,
									..
								} => (*session_id, device_info.device_id),
								super::behavior::PairingMessage::Response {
									session_id,
									device_info,
									..
								} => (*session_id, device_info.device_id),
								super::behavior::PairingMessage::Challenge {
									session_id,
									device_info,
									..
								} => (*session_id, device_info.device_id),
								super::behavior::PairingMessage::Complete {
									session_id, ..
								} => {
									// For complete messages, lookup the device ID from existing mappings
									let registry = device_registry.read().await;
									let device_id = registry
										.get_device_by_session(*session_id)
										.or_else(|| registry.get_device_by_peer(peer))
										.unwrap_or_else(Uuid::new_v4);
									drop(registry);
									(*session_id, device_id)
								}
							};

							// Check if we already know this peer
							let existing_device_id =
								device_registry.read().await.get_device_by_peer(peer);

							let device_id =
								if let Some(existing_id) = existing_device_id {
									println!(
										"Using existing device ID {} for peer {}",
										existing_id, peer
									);
									existing_id
								} else {
									// Register this new pairing relationship in the device registry
									println!("Registering new pairing: device {} with peer {} for session {}",
									device_id_from_request, peer, session_id);

									match device_registry.write().await.start_pairing(
										device_id_from_request,
										peer,
										session_id,
									) {
										Ok(()) => {
											println!("Successfully registered pairing in device registry");
											device_id_from_request
										}
										Err(e) => {
											eprintln!(
												"Failed to register pairing in device registry: {}",
												e
											);
											device_id_from_request // Use the device ID from request anyway
										}
									}
								};

							// Handle the request through the protocol registry
							match protocol_registry
								.read()
								.await
								.handle_request(
									"pairing",
									device_id,
									serde_json::to_vec(&request).unwrap_or_default(),
								)
								.await
							{
								Ok(response_data) => {
									// Deserialize response back to PairingMessage for LibP2P
									if let Ok(response_message) = serde_json::from_slice::<
										super::behavior::PairingMessage,
									>(&response_data)
									{
										// Send response back through LibP2P
										if let Err(e) = swarm
											.behaviour_mut()
											.pairing
											.send_response(channel, response_message)
										{
											eprintln!("Failed to send pairing response: {:?}", e);
										} else {
											println!("Sent pairing response to {}", peer);
										}
									} else {
										eprintln!("Failed to deserialize pairing response");
									}
								}
								Err(e) => {
									eprintln!("Protocol handler error: {}", e);
								}
							}
						}
						request_response::Message::Response { response, .. } => {
							println!("Received pairing response from {}", peer);

							// Get device ID from peer ID (or use placeholder if not found)
							let device_id = device_registry
								.read()
								.await
								.get_device_by_peer(peer)
								.unwrap_or_else(|| Uuid::new_v4());

							// Handle the response through the protocol registry
							if let Err(e) = protocol_registry
								.read()
								.await
								.handle_response(
									"pairing",
									device_id,
									peer,
									serde_json::to_vec(&response).unwrap_or_default(),
								)
								.await
							{
								eprintln!("Protocol handler error handling response: {}", e);
							}
						}
					},
					_ => {}
				}
			}

			UnifiedBehaviourEvent::Messaging(req_resp_event) => {
				use libp2p::request_response;
				match req_resp_event {
					request_response::Event::Message {
						peer,
						message,
						connection_id: _,
					} => match message {
						request_response::Message::Request { request, .. } => {
							println!("Received message request from {}", peer);
							// TODO: Implement messaging protocol handler similar to pairing
							// For now, just log the received message
						}
						request_response::Message::Response { response, .. } => {
							println!("Received message response from {}", peer);

							let _ = protocol_registry
								.read()
								.await
								.handle_response(
									"messaging",
									Uuid::new_v4(),
									peer,
									serde_json::to_vec(&response).unwrap_or_default(),
								)
								.await;
						}
					},
					_ => {}
				}
			}

			UnifiedBehaviourEvent::FileTransfer(req_resp_event) => {
				use libp2p::request_response;
				match req_resp_event {
					request_response::Event::Message {
						peer,
						message,
						connection_id: _,
					} => match message {
						request_response::Message::Request {
							request,
							channel,
							request_id: _,
						} => {
							println!("🔄 Received file transfer request from {}", peer);

							// Get device ID from device registry using peer ID
							let device_id =
								match device_registry.read().await.get_device_by_peer(peer) {
									Some(id) => {
										println!(
											"🔗 File Transfer: Found device {} for peer {}",
											id, peer
										);
										id
									}
									None => {
										eprintln!(
											"❌ File Transfer: No device mapping found for peer {}",
											peer
										);
										return Ok(()); // Skip processing this request
									}
								};

							// Handle the request through the protocol registry
							match protocol_registry
								.read()
								.await
								.handle_request(
									"file_transfer",
									device_id,
									rmp_serde::to_vec(&request).unwrap_or_default(),
								)
								.await
							{
								Ok(response_data) => {
									// Deserialize response back to FileTransferMessage for LibP2P
									if let Ok(response_message) = rmp_serde::from_slice::<
										super::behavior::FileTransferMessage,
									>(&response_data)
									{
										// Send response back through LibP2P
										if let Err(e) = swarm
											.behaviour_mut()
											.file_transfer
											.send_response(channel, response_message)
										{
											eprintln!(
												"Failed to send file transfer response: {:?}",
												e
											);
										} else {
											println!("✅ Sent file transfer response to {}", peer);
										}
									} else {
										eprintln!(
											"❌ Failed to deserialize file transfer response"
										);
									}
								}
								Err(e) => {
									eprintln!("❌ File transfer protocol handler error: {}", e);
								}
							}
						}
						request_response::Message::Response { response, .. } => {
							println!("✅ Received file transfer response from {}", peer);

							let _ = protocol_registry
								.read()
								.await
								.handle_response(
									"file_transfer",
									Uuid::new_v4(),
									peer,
									rmp_serde::to_vec(&response).unwrap_or_default(),
								)
								.await;
						}
					},
					_ => {}
				}
			}
		}

		Ok(())
	}
}

// Ensure event loop is Send + Sync
unsafe impl Send for NetworkingEventLoop {}
unsafe impl Sync for NetworkingEventLoop {}
