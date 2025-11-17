//! Basic messaging protocol handler

use super::{library_messages::LibraryMessage, ProtocolEvent, ProtocolHandler};
use crate::service::network::{utils, NetworkingError, Result};
use async_trait::async_trait;
use iroh::{endpoint::Connection, Endpoint, NodeAddr, NodeId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Basic messaging protocol handler
pub struct MessagingProtocolHandler {
	/// Optional context for accessing libraries
	context: Option<Arc<crate::context::CoreContext>>,

	/// Device registry for node_id → device_id mapping
	device_registry: Arc<RwLock<crate::service::network::device::DeviceRegistry>>,

	/// Endpoint for creating and managing connections
	endpoint: Option<Endpoint>,

	/// Cached connections to remote nodes (keyed by NodeId and ALPN)
	connections: Arc<RwLock<HashMap<(NodeId, Vec<u8>), Connection>>>,
}

/// Basic message types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
	/// Ping message for connection testing
	Ping {
		timestamp: chrono::DateTime<chrono::Utc>,
		payload: Option<Vec<u8>>,
	},
	/// Pong response
	Pong {
		timestamp: chrono::DateTime<chrono::Utc>,
		original_timestamp: chrono::DateTime<chrono::Utc>,
	},
	/// Generic data message
	Data {
		message_id: Uuid,
		content_type: String,
		payload: Vec<u8>,
	},
	/// Acknowledgment message
	Ack {
		message_id: Uuid,
		success: bool,
		error: Option<String>,
	},
	/// Graceful disconnect notification
	Goodbye {
		reason: String,
		timestamp: chrono::DateTime<chrono::Utc>,
	},
	/// Library-related message
	Library(LibraryMessage),
}

impl MessagingProtocolHandler {
	/// Create a new messaging protocol handler with shared connection cache
	///
	/// Uses the same connection cache as other protocols (pairing, sync, file transfer)
	/// to follow Iroh best practice of one persistent connection per device pair
	pub fn new(
		device_registry: Arc<RwLock<crate::service::network::device::DeviceRegistry>>,
		endpoint: Option<Endpoint>,
		active_connections: Arc<RwLock<HashMap<(NodeId, Vec<u8>), Connection>>>,
	) -> Self {
		Self {
			context: None,
			device_registry,
			endpoint,
			connections: active_connections,
		}
	}

	/// Set context after creation
	pub fn set_context(&mut self, context: Arc<crate::context::CoreContext>) {
		self.context = Some(context);
	}

	async fn handle_ping(
		&self,
		_from_device: Uuid,
		timestamp: chrono::DateTime<chrono::Utc>,
		_payload: Option<Vec<u8>>,
	) -> Result<Vec<u8>> {
		let response = Message::Pong {
			timestamp: chrono::Utc::now(),
			original_timestamp: timestamp,
		};

		serde_json::to_vec(&response).map_err(|e| NetworkingError::Serialization(e))
	}

	async fn handle_pong(
		&self,
		_from_device: Uuid,
		_timestamp: chrono::DateTime<chrono::Utc>,
		original_timestamp: chrono::DateTime<chrono::Utc>,
	) -> Result<Vec<u8>> {
		let now = chrono::Utc::now();
		let rtt = now.signed_duration_since(original_timestamp);

		println!("Ping RTT: {}ms", rtt.num_milliseconds());

		// Return empty response for pong
		Ok(Vec::new())
	}

	async fn handle_data(
		&self,
		from_device: Uuid,
		message_id: Uuid,
		content_type: String,
		payload: Vec<u8>,
	) -> Result<Vec<u8>> {
		// Process the data message
		println!(
			"Received data message from {}: {} ({} bytes)",
			from_device,
			content_type,
			payload.len()
		);

		// Send acknowledgment
		let response = Message::Ack {
			message_id,
			success: true,
			error: None,
		};

		serde_json::to_vec(&response).map_err(|e| NetworkingError::Serialization(e))
	}

	async fn handle_ack(
		&self,
		_from_device: Uuid,
		message_id: Uuid,
		success: bool,
		error: Option<String>,
	) -> Result<Vec<u8>> {
		if success {
			println!("Message {} acknowledged successfully", message_id);
		} else {
			println!("Message {} failed: {:?}", message_id, error);
		}

		// Return empty response for ack
		Ok(Vec::new())
	}

	async fn handle_library_message(
		&self,
		_from_device: Uuid,
		library_msg: LibraryMessage,
	) -> Result<Vec<u8>> {
		use super::library_messages::{LibraryDiscoveryInfo, LibraryMessage};

		match library_msg {
			LibraryMessage::DiscoveryRequest { request_id } => {
				// Get context to access libraries
				let context = self.context.as_ref().ok_or_else(|| {
					NetworkingError::Protocol(
						"Context not available for library operations".to_string(),
					)
				})?;

				let library_manager = context.libraries().await;
				let libraries = library_manager.list().await;

				// Convert to discovery info
				let mut library_infos = Vec::new();
				for library in libraries {
					let library_id = library.id();
					let name = library.name().await;
					let config_guard = library.config().await;

					// Get library statistics from database
					let db = library.db();
					use crate::infra::db::entities;
					use sea_orm::{EntityTrait, PaginatorTrait};

					let entry_count = match entities::entry::Entity::find().count(db.conn()).await {
						Ok(count) => count,
						Err(_) => 0,
					};

					let location_count =
						match entities::location::Entity::find().count(db.conn()).await {
							Ok(count) => count,
							Err(_) => 0,
						};

					let device_count = match entities::device::Entity::find().count(db.conn()).await
					{
						Ok(count) => count,
						Err(_) => 0,
					};

					library_infos.push(LibraryDiscoveryInfo {
						id: library_id,
						name,
						description: config_guard.description.clone(),
						created_at: config_guard.created_at,
						total_entries: entry_count,
						total_locations: location_count,
						total_size_bytes: 0, // TODO: Calculate from entries
						device_count,
					});
				}

				let response = Message::Library(LibraryMessage::DiscoveryResponse {
					request_id,
					libraries: library_infos,
				});

				serde_json::to_vec(&response).map_err(|e| NetworkingError::Serialization(e))
			}

			LibraryMessage::RegisterDeviceRequest {
				request_id,
				library_id,
				device_id,
				device_name,
				device_slug,
				os_name,
				os_version,
				hardware_model,
			} => {
				// Get context
				let context = self.context.as_ref().ok_or_else(|| {
					NetworkingError::Protocol(
						"Context not available for library operations".to_string(),
					)
				})?;

				let library_manager = context.libraries().await;

				// Determine which library to register device in
				let libraries = if let Some(lib_id) = library_id {
					// Specific library
					vec![library_manager.get_library(lib_id).await.ok_or_else(|| {
						NetworkingError::Protocol(format!("Library not found: {}", lib_id))
					})?]
				} else {
					// All libraries
					library_manager.list().await
				};

				// Register device in each library
				use crate::infra::db::entities;
				use chrono::Utc;
				use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};

				let mut success = true;
				let mut error_msg = None;

				for library in libraries {
					let db = library.db();

					// Check if device already exists
					let existing = entities::device::Entity::find()
						.filter(entities::device::Column::Uuid.eq(device_id))
						.one(db.conn())
						.await;

					match existing {
						Ok(Some(_)) => {
							// Already registered, skip
							continue;
						}
						Ok(None) => {
							// Get existing slugs for collision detection
							let existing_slugs: Vec<String> =
								match entities::device::Entity::find().all(db.conn()).await {
									Ok(devices) => devices.iter().map(|d| d.slug.clone()).collect(),
									Err(e) => {
										success = false;
										error_msg = Some(format!("Database error: {}", e));
										break;
									}
								};

							// Check if the device's slug conflicts and rename if needed
							let unique_slug = crate::library::Library::ensure_unique_slug(
								&device_slug,
								&existing_slugs,
							);

							if unique_slug != device_slug {
								tracing::info!(
									"Device slug collision in library {}. Registering device as '{}' instead of '{}'",
									library.id(),
									unique_slug,
									device_slug
								);
							}

							// Register remote device
							let device_model = entities::device::ActiveModel {
								id: sea_orm::ActiveValue::NotSet,
								uuid: Set(device_id),
								name: Set(device_name.clone()),
								slug: Set(unique_slug),
								os: Set(os_name.clone()),
								os_version: Set(os_version.clone()),
								hardware_model: Set(hardware_model.clone()),
								network_addresses: Set(serde_json::json!([])),
								is_online: Set(false),
								last_seen_at: Set(Utc::now()),
								capabilities: Set(serde_json::json!({
									"indexing": true,
									"p2p": true,
									"volume_detection": true
								})),
								created_at: Set(Utc::now()),
								sync_enabled: Set(true), // Enable sync for registered devices
								last_sync_at: Set(None),
								updated_at: Set(Utc::now()),
							};

							if let Err(e) = device_model.insert(db.conn()).await {
								success = false;
								error_msg = Some(format!("Failed to register device: {}", e));
								break;
							}
						}
						Err(e) => {
							success = false;
							error_msg = Some(format!("Database error: {}", e));
							break;
						}
					}
				}

				let response = Message::Library(LibraryMessage::RegisterDeviceResponse {
					request_id,
					success,
					message: error_msg,
				});

				serde_json::to_vec(&response).map_err(|e| NetworkingError::Serialization(e))
			}

			LibraryMessage::DiscoveryResponse { .. }
			| LibraryMessage::RegisterDeviceResponse { .. } => {
				// These are responses, not requests
				Ok(Vec::new())
			}

			LibraryMessage::CreateSharedLibraryRequest {
				request_id,
				library_id,
				library_name,
				description,
				requesting_device_id,
				requesting_device_name,
				requesting_device_slug,
			} => {
				tracing::info!(
					"Received CreateSharedLibraryRequest: {} ({}) from device {} (slug: {})",
					library_name,
					library_id,
					requesting_device_id,
					requesting_device_slug
				);

				let context = self.context.as_ref().ok_or_else(|| {
					NetworkingError::Protocol("Context not available".to_string())
				})?;

				let library_manager = context.libraries().await;

				// Check if library already exists
				if let Some(existing_library) = library_manager.get_library(library_id).await {
					tracing::info!("Library {} already exists, returning success", library_id);

					// Get this device's slug in the library
					let device_slug = context.device_manager.slug_for_library(library_id).ok();

					let response = Message::Library(LibraryMessage::CreateSharedLibraryResponse {
						request_id,
						success: true,
						message: Some("Library already exists".to_string()),
						device_slug,
					});
					return serde_json::to_vec(&response)
						.map_err(|e| NetworkingError::Serialization(e));
				}

				// Create library with specific UUID
				// Note: We pass the requesting device info so it can be pre-registered
				// before ensure_device_registered runs for the current device
				match library_manager
					.create_library_with_id_and_initial_device(
						library_id,
						library_name.clone(),
						description,
						requesting_device_id,
						requesting_device_name,
						requesting_device_slug,
						context.clone(),
					)
					.await
				{
					Ok(_) => {
						tracing::info!("Successfully created shared library: {}", library_name);

						// Get this device's resolved slug in the new library
						// After ensure_device_registered, this will return the collision-resolved slug
						let device_slug = context.device_manager.slug_for_library(library_id).ok();

						let response =
							Message::Library(LibraryMessage::CreateSharedLibraryResponse {
								request_id,
								success: true,
								message: None,
								device_slug,
							});
						serde_json::to_vec(&response).map_err(|e| NetworkingError::Serialization(e))
					}
					Err(e) => {
						tracing::error!("Failed to create library: {}", e);
						let response =
							Message::Library(LibraryMessage::CreateSharedLibraryResponse {
								request_id,
								success: false,
								message: Some(e.to_string()),
								device_slug: None,
							});
						serde_json::to_vec(&response).map_err(|e| NetworkingError::Serialization(e))
					}
				}
			}

			LibraryMessage::CreateSharedLibraryResponse { .. } => {
				// This is a response, not a request
				Ok(Vec::new())
			}

			LibraryMessage::LibraryStateRequest {
				request_id,
				library_id,
			} => {
				tracing::info!("Received LibraryStateRequest for library {}", library_id);

				let context = self.context.as_ref().ok_or_else(|| {
					NetworkingError::Protocol("Context not available".to_string())
				})?;

				let library_manager = context.libraries().await;
				let library = library_manager
					.get_library(library_id)
					.await
					.ok_or_else(|| {
						NetworkingError::Protocol(format!("Library {} not found", library_id))
					})?;

				let db = library.db();

				// Query all device slugs from this library
				use crate::infra::db::entities;
				use sea_orm::EntityTrait;

				let devices = entities::device::Entity::find()
					.all(db.conn())
					.await
					.map_err(|e| NetworkingError::Protocol(format!("Database error: {}", e)))?;

				let device_slugs: Vec<String> = devices.iter().map(|d| d.slug.clone()).collect();

				tracing::info!(
					"Returning {} device slugs for library {}",
					device_slugs.len(),
					library_id
				);

				let response = Message::Library(LibraryMessage::LibraryStateResponse {
					request_id,
					library_id,
					library_name: library.name().await,
					device_slugs,
					device_count: devices.len(),
				});

				serde_json::to_vec(&response).map_err(|e| NetworkingError::Serialization(e))
			}

			LibraryMessage::LibraryStateResponse { .. } => {
				// This is a response, not a request
				Ok(Vec::new())
			}
		}
	}

	/// Send a library message to a remote node and wait for response
	/// Uses cached connections and creates new streams (Iroh best practice)
	pub async fn send_library_message(
		&self,
		node_id: NodeId,
		message: LibraryMessage,
	) -> Result<LibraryMessage> {
		use tokio::io::{AsyncReadExt, AsyncWriteExt};
		use tokio::time::{timeout, Duration};

		tracing::info!("Sending library message to node {}: {:?}", node_id, message);

		// Get or create cached connection
		let endpoint = self.endpoint.as_ref().ok_or_else(|| {
			NetworkingError::ConnectionFailed("No endpoint available".to_string())
		})?;

		let logger: Arc<dyn utils::NetworkLogger> = Arc::new(utils::SilentLogger);
		let conn = utils::get_or_create_connection(
			self.connections.clone(),
			endpoint,
			node_id,
			crate::service::network::core::MESSAGING_ALPN,
			&logger,
		)
		.await?;

		// Create new stream on existing connection
		let (mut send, mut recv) = conn.open_bi().await.map_err(|e| {
			NetworkingError::ConnectionFailed(format!("Failed to open stream: {}", e))
		})?;

		// Wrap message in envelope
		let envelope = Message::Library(message);
		let msg_data =
			serde_json::to_vec(&envelope).map_err(|e| NetworkingError::Serialization(e))?;

		// Send with length prefix
		let len = msg_data.len() as u32;
		send.write_all(&len.to_be_bytes())
			.await
			.map_err(|e| NetworkingError::Transport(format!("Failed to send length: {}", e)))?;
		send.write_all(&msg_data)
			.await
			.map_err(|e| NetworkingError::Transport(format!("Failed to send data: {}", e)))?;

		// Properly close stream (Iroh best practice)
		send.finish()
			.map_err(|e| NetworkingError::Transport(format!("Failed to finish stream: {}", e)))?;

		tracing::debug!("Message sent, waiting for response...");

		// Read response with timeout
		let result = timeout(Duration::from_secs(30), async {
			let mut len_buf = [0u8; 4];
			recv.read_exact(&mut len_buf).await.map_err(|e| {
				NetworkingError::Transport(format!("Failed to read response length: {}", e))
			})?;
			let resp_len = u32::from_be_bytes(len_buf) as usize;

			tracing::debug!("Receiving response of {} bytes", resp_len);

			let mut resp_buf = vec![0u8; resp_len];
			recv.read_exact(&mut resp_buf).await.map_err(|e| {
				NetworkingError::Transport(format!("Failed to read response: {}", e))
			})?;
			Ok::<_, NetworkingError>(resp_buf)
		})
		.await;

		let resp_buf = match result {
			Ok(Ok(buf)) => buf,
			Ok(Err(e)) => return Err(e),
			Err(_) => {
				return Err(NetworkingError::Transport(
					"Request timed out after 30s".to_string(),
				))
			}
		};

		// Deserialize response
		let envelope: Message =
			serde_json::from_slice(&resp_buf).map_err(|e| NetworkingError::Serialization(e))?;

		match envelope {
			Message::Library(lib_msg) => {
				tracing::debug!("Received library message response: {:?}", lib_msg);
				Ok(lib_msg)
			}
			_ => Err(NetworkingError::Protocol(
				"Expected Library message in response".to_string(),
			)),
		}
	}
}

#[async_trait]
impl ProtocolHandler for MessagingProtocolHandler {
	fn protocol_name(&self) -> &str {
		"messaging"
	}

	fn as_any(&self) -> &dyn std::any::Any {
		self
	}

	async fn handle_stream(
		&self,
		mut send: Box<dyn tokio::io::AsyncWrite + Send + Unpin>,
		mut recv: Box<dyn tokio::io::AsyncRead + Send + Unpin>,
		remote_node_id: NodeId,
	) {
		use tokio::io::{AsyncReadExt, AsyncWriteExt};

		// Simple request-response messaging over streams
		loop {
			// Read message length (4 bytes)
			let mut len_buf = [0u8; 4];
			match recv.read_exact(&mut len_buf).await {
				Ok(_) => {}
				Err(_) => break, // Connection closed
			}
			let msg_len = u32::from_be_bytes(len_buf) as usize;

			// Read message
			let mut msg_buf = vec![0u8; msg_len];
			if let Err(e) = recv.read_exact(&mut msg_buf).await {
				eprintln!("Failed to read message: {}", e);
				break;
			}

			// Deserialize and handle
			match serde_json::from_slice::<Message>(&msg_buf) {
				Ok(message) => {
					// Process message based on type
					let response = match &message {
						Message::Ping { timestamp, payload } => {
							let pong = Message::Pong {
								timestamp: chrono::Utc::now(),
								original_timestamp: *timestamp,
							};
							serde_json::to_vec(&pong).unwrap_or_default()
						}
						Message::Data { message_id, .. } => {
							let ack = Message::Ack {
								message_id: *message_id,
								success: true,
								error: None,
							};
							serde_json::to_vec(&ack).unwrap_or_default()
						}
						Message::Library(lib_msg) => {
							// Handle library message - need to derive device_id from node_id
							// For now, use a placeholder (TODO: proper mapping)
							// Map node_id to device_id using registry
							let device_id_opt = {
								let registry = self.device_registry.read().await;
								registry.get_device_by_node(remote_node_id)
							};

							let resp = match device_id_opt {
								Some(device_id) => {
									match self
										.handle_library_message(device_id, lib_msg.clone())
										.await
									{
										Ok(resp) => resp,
										Err(e) => {
											tracing::error!(
												"Failed to handle library message: {}",
												e
											);
											Vec::new()
										}
									}
								}
								None => {
									tracing::warn!(
										"Received library message from unknown node {}",
										remote_node_id
									);
									Vec::new()
								}
							};
							resp
						}
						Message::Goodbye { reason, .. } => {
							// Received graceful disconnect from remote device
							eprintln!("Remote device disconnecting gracefully: {}", reason);
							// Close the stream by breaking the loop
							break;
						}
						_ => Vec::new(), // No response for Pong/Ack
					};

					// Send response if any
					if !response.is_empty() {
						let len = response.len() as u32;
						if send.write_all(&len.to_be_bytes()).await.is_err() {
							break;
						}
						if send.write_all(&response).await.is_err() {
							break;
						}
						let _ = send.flush().await;
					}
				}
				Err(e) => {
					eprintln!("Failed to deserialize message: {}", e);
					break;
				}
			}
		}
	}

	async fn handle_request(&self, from_device: Uuid, request_data: Vec<u8>) -> Result<Vec<u8>> {
		let message: Message =
			serde_json::from_slice(&request_data).map_err(|e| NetworkingError::Serialization(e))?;

		match message {
			Message::Ping { timestamp, payload } => {
				self.handle_ping(from_device, timestamp, payload).await
			}
			Message::Pong {
				timestamp,
				original_timestamp,
			} => {
				self.handle_pong(from_device, timestamp, original_timestamp)
					.await
			}
			Message::Data {
				message_id,
				content_type,
				payload,
			} => {
				self.handle_data(from_device, message_id, content_type, payload)
					.await
			}
			Message::Ack {
				message_id,
				success,
				error,
			} => {
				self.handle_ack(from_device, message_id, success, error)
					.await
			}
			Message::Library(lib_msg) => self.handle_library_message(from_device, lib_msg).await,
			Message::Goodbye { reason, .. } => {
				println!(
					"Device {} disconnecting gracefully: {}",
					from_device, reason
				);
				// Return empty response, connection will be closed by the sender
				Ok(Vec::new())
			}
		}
	}

	async fn handle_response(
		&self,
		_from_device: Uuid,
		_from_node: NodeId,
		_response_data: Vec<u8>,
	) -> Result<()> {
		// Messaging protocol handles responses in handle_request
		Ok(())
	}

	async fn handle_event(&self, _event: ProtocolEvent) -> Result<()> {
		// Basic messaging doesn't need special event handling
		Ok(())
	}
}
