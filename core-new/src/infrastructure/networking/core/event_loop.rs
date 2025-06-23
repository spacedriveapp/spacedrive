//! Central event loop for processing LibP2P events

use super::{
	behavior::{UnifiedBehaviour, UnifiedBehaviourEvent},
	NetworkEvent,
};
use crate::infrastructure::networking::{
	device::{DeviceConnection, DeviceInfo, DeviceRegistry},
	protocols::{ProtocolEvent, ProtocolRegistry},
	NetworkingError, Result,
};
use futures::StreamExt;
use libp2p::{
	kad::{self, QueryId, RecordKey},
	swarm::{Swarm, SwarmEvent},
	Multiaddr, PeerId,
};
use std::sync::Arc;
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
}

impl NetworkingEventLoop {
	/// Create a new event loop
	pub fn new(
		swarm: Swarm<UnifiedBehaviour>,
		protocol_registry: Arc<RwLock<ProtocolRegistry>>,
		device_registry: Arc<RwLock<DeviceRegistry>>,
		event_sender: mpsc::UnboundedSender<NetworkEvent>,
	) -> Self {
		let (shutdown_sender, shutdown_receiver) = mpsc::unbounded_channel();
		let (command_sender, command_receiver) = mpsc::unbounded_channel();

		Self {
			swarm,
			protocol_registry,
			device_registry,
			event_sender,
			shutdown_receiver: Some(shutdown_receiver),
			shutdown_sender,
			command_receiver: Some(command_receiver),
			command_sender,
			is_running: false,
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

		// Move swarm into the task
		let mut swarm = self.swarm;

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
						_ => {
							println!("Unknown protocol: {}", protocol);
						}
					}
				} else {
					println!("Device {} not found or not connected", device_id);
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
				match swarm.behaviour_mut().kademlia.put_record(record, kad::Quorum::One) {
					Ok(query_id) => {
						println!("Publishing DHT record with query ID: {:?}", query_id);
						let _ = response_channel.send(Ok(query_id));
					}
					Err(e) => {
						println!("Failed to publish DHT record: {:?}", e);
						let _ = response_channel.send(Err(NetworkingError::Protocol(format!("DHT put failed: {:?}", e))));
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
		}

		Ok(())
	}

	/// Handle a single swarm event
	async fn handle_swarm_event(
		event: SwarmEvent<UnifiedBehaviourEvent>,
		protocol_registry: &Arc<RwLock<ProtocolRegistry>>,
		device_registry: &Arc<RwLock<DeviceRegistry>>,
		event_sender: &mpsc::UnboundedSender<NetworkEvent>,
	) -> Result<()> {
		match event {
			SwarmEvent::NewListenAddr { address, .. } => {
				println!("Listening on: {}", address);
			}

			SwarmEvent::ConnectionEstablished { peer_id, .. } => {
				println!("Connection established with: {}", peer_id);

				// Look up device by peer ID
				if let Some(device_id) = device_registry.read().await.get_device_by_peer(peer_id) {
					let _ = event_sender
						.send(NetworkEvent::ConnectionEstablished { device_id, peer_id });
				}
			}

			SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
				println!("Connection closed with {}: {:?}", peer_id, cause);

				// Look up device by peer ID
				if let Some(device_id) = device_registry.read().await.get_device_by_peer(peer_id) {
					let _ = device_registry.write().await.mark_disconnected(
                        device_id,
                        crate::infrastructure::networking::device::DisconnectionReason::NetworkError(
                            format!("{:?}", cause)
                        ),
                    );

					let _ = event_sender.send(NetworkEvent::ConnectionLost { device_id, peer_id });
				}
			}

			SwarmEvent::Behaviour(behaviour_event) => {
				Self::handle_behaviour_event(
					behaviour_event,
					protocol_registry,
					device_registry,
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

	/// Handle behavior-specific events
	async fn handle_behaviour_event(
		event: UnifiedBehaviourEvent,
		protocol_registry: &Arc<RwLock<ProtocolRegistry>>,
		device_registry: &Arc<RwLock<DeviceRegistry>>,
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
					} => {
						match put_result {
							Ok(kad::PutRecordOk { key }) => {
								println!("DHT record published successfully: query_id={:?}, key={:?}", id, key);
							}
							Err(kad::PutRecordError::QuorumFailed { key, success, quorum }) => {
								println!("DHT record publish failed: query_id={:?}, key={:?}, success={:?}, quorum={:?}", id, key, success, quorum);
							}
							Err(kad::PutRecordError::Timeout { key, .. }) => {
								println!("DHT record publish timed out: query_id={:?}, key={:?}", id, key);
							}
						}
					}
					kad::Event::OutboundQueryProgressed {
						id,
						result: kad::QueryResult::GetRecord(get_result),
						..
					} => {
						match get_result {
							Ok(kad::GetRecordOk::FoundRecord(record)) => {
								println!("DHT record found: query_id={:?}, key={:?}, {} bytes", id, record.record.key, record.record.value.len());
								
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
								println!("DHT query finished, no additional records: query_id={:?}", id);
							}
							Err(kad::GetRecordError::NotFound { key, .. }) => {
								println!("DHT record not found: query_id={:?}, key={:?}", id, key);
							}
							Err(kad::GetRecordError::QuorumFailed { key, .. }) => {
								println!("DHT query quorum failed: query_id={:?}, key={:?}", id, key);
							}
							Err(kad::GetRecordError::Timeout { key }) => {
								println!("DHT query timed out: query_id={:?}, key={:?}", id, key);
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
							channel: _,
							request_id,
						} => {
							println!("Received pairing request from {}", peer);

							if let Ok(_response_data) = protocol_registry
								.read()
								.await
								.handle_request(
									"pairing",
									Uuid::new_v4(),
									serde_json::to_vec(&request).unwrap_or_default(),
								)
								.await
							{
								println!("Sending pairing response");
							}
						}
						request_response::Message::Response { response, .. } => {
							println!("Received pairing response from {}", peer);

							let _ = protocol_registry
								.read()
								.await
								.handle_response(
									"pairing",
									Uuid::new_v4(),
									serde_json::to_vec(&response).unwrap_or_default(),
								)
								.await;
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

							if let Ok(_response_data) = protocol_registry
								.read()
								.await
								.handle_request(
									"messaging",
									Uuid::new_v4(),
									serde_json::to_vec(&request).unwrap_or_default(),
								)
								.await
							{
								println!("Sending message response");
							}
						}
						request_response::Message::Response { response, .. } => {
							println!("Received message response from {}", peer);

							let _ = protocol_registry
								.read()
								.await
								.handle_response(
									"messaging",
									Uuid::new_v4(),
									serde_json::to_vec(&response).unwrap_or_default(),
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
