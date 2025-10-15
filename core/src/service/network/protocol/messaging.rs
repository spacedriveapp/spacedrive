//! Basic messaging protocol handler

use super::{library_messages::LibraryMessage, ProtocolEvent, ProtocolHandler};
use crate::service::network::{NetworkingError, Result};
use async_trait::async_trait;
use iroh::NodeId;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

/// Basic messaging protocol handler
pub struct MessagingProtocolHandler {
	/// Optional context for accessing libraries
	context: Option<Arc<crate::context::CoreContext>>,
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
	/// Create a new messaging protocol handler
	pub fn new() -> Self {
		Self { context: None }
	}

	/// Create handler with context for library operations
	pub fn with_context(context: Arc<crate::context::CoreContext>) -> Self {
		Self {
			context: Some(context),
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
							// Register new device
							let device_model = entities::device::ActiveModel {
								id: sea_orm::ActiveValue::NotSet,
								uuid: Set(device_id),
								name: Set(device_name.clone()),
								slug: Set(crate::domain::device::Device::generate_slug(&device_name)),
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
								last_state_watermark: Set(None),
								last_shared_watermark: Set(None),
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
		}
	}
}

impl Default for MessagingProtocolHandler {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl ProtocolHandler for MessagingProtocolHandler {
	fn protocol_name(&self) -> &str {
		"messaging"
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
							let device_id = Uuid::nil();
							match self
								.handle_library_message(device_id, lib_msg.clone())
								.await
							{
								Ok(resp) => resp,
								Err(e) => {
									eprintln!("Failed to handle library message: {}", e);
									Vec::new()
								}
							}
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

	fn as_any(&self) -> &dyn std::any::Any {
		self
	}
}
