//! Sync Protocol Handler (Leaderless Hybrid Architecture)
//!
//! Handles incoming sync messages and routes them to the appropriate
//! PeerSync methods for processing.

use super::messages::{StateRecord, SyncMessage};
use crate::service::{
	network::{NetworkingError, Result},
	sync::state::StateChangeMessage,
};
use async_trait::async_trait;
use std::sync::Arc;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Sync protocol handler for leaderless hybrid sync
///
/// Routes incoming sync messages to PeerSync for processing.
pub struct SyncProtocolHandler {
	library_id: Uuid,
	peer_sync: Option<Arc<crate::service::sync::peer::PeerSync>>,
}

impl SyncProtocolHandler {
	/// Create a new sync protocol handler
	pub fn new(library_id: Uuid) -> Self {
		info!(
			library_id = %library_id,
			"Creating SyncProtocolHandler for leaderless hybrid sync"
		);
		Self {
			library_id,
			peer_sync: None,
		}
	}

	/// Set the peer sync service (called after initialization)
	pub fn set_peer_sync(&mut self, peer_sync: Arc<crate::service::sync::peer::PeerSync>) {
		self.peer_sync = Some(peer_sync);
	}

	/// Get library ID
	pub fn library_id(&self) -> Uuid {
		self.library_id
	}

	/// Handle an incoming sync message
	async fn handle_sync_message(
		&self,
		from_device: Uuid,
		message: SyncMessage,
	) -> Result<Option<SyncMessage>> {
		let peer_sync = self
			.peer_sync
			.as_ref()
			.ok_or_else(|| NetworkingError::Protocol("PeerSync not initialized".to_string()))?;

		debug!(
			from_device = %from_device,
			library_id = %message.library_id(),
			"Processing sync message"
		);

		match message {
			SyncMessage::StateChange {
				library_id,
				model_type,
				record_uuid,
				device_id,
				data,
				timestamp,
			} => {
				let change = StateChangeMessage {
					model_type,
					record_uuid,
					device_id,
					data,
					timestamp,
				};

				peer_sync
					.on_state_change_received(change)
					.await
					.map_err(|e| {
						NetworkingError::Protocol(format!("Failed to apply state change: {}", e))
					})?;

				Ok(None) // No response needed
			}

			SyncMessage::StateBatch {
				library_id,
				model_type,
				device_id,
				records,
			} => {
				info!(
					from_device = %from_device,
					model_type = %model_type,
					count = records.len(),
					"Processing state batch"
				);

				for record in records {
					let change = StateChangeMessage {
						model_type: model_type.clone(),
						record_uuid: record.uuid,
						device_id,
						data: record.data,
						timestamp: record.timestamp,
					};

					peer_sync
						.on_state_change_received(change)
						.await
						.map_err(|e| {
							NetworkingError::Protocol(format!(
								"Failed to apply state in batch: {}",
								e
							))
						})?;
				}

				Ok(None)
			}

			SyncMessage::SharedChange { library_id, entry } => {
				peer_sync
					.on_shared_change_received(entry)
					.await
					.map_err(|e| {
						NetworkingError::Protocol(format!("Failed to apply shared change: {}", e))
					})?;

				Ok(None)
			}

			SyncMessage::SharedChangeBatch {
				library_id,
				entries,
			} => {
				info!(
					from_device = %from_device,
					count = entries.len(),
					"Processing shared change batch"
				);

				for entry in entries {
					peer_sync
						.on_shared_change_received(entry)
						.await
						.map_err(|e| {
							NetworkingError::Protocol(format!(
								"Failed to apply shared change in batch: {}",
								e
							))
						})?;
				}

				Ok(None)
			}

			SyncMessage::StateRequest {
				library_id,
				model_types,
				device_id,
				since,
				checkpoint,
				batch_size,
			} => {
				debug!(
					model_types = ?model_types,
					device_id = ?device_id,
					batch_size = batch_size,
					"Processing StateRequest"
				);

				// Query local state
				let records = peer_sync
					.get_device_state(model_types.clone(), device_id, since, batch_size)
					.await
					.map_err(|e| {
						NetworkingError::Protocol(format!("Failed to query device state: {}", e))
					})?;

				let has_more = records.len() >= batch_size;
				let model_type = model_types.first().cloned().unwrap_or_default();

				Ok(Some(SyncMessage::StateResponse {
					library_id,
					model_type,
					device_id: device_id.unwrap_or(from_device),
					records,
					checkpoint: None, // TODO: Implement checkpoint tracking
					has_more,
				}))
			}

			SyncMessage::StateResponse { .. } => {
				// Response messages are handled separately
				Ok(None)
			}

			SyncMessage::SharedChangeRequest {
				library_id,
				since_hlc,
				limit,
			} => {
				debug!(
					since_hlc = ?since_hlc,
					limit = limit,
					"Processing SharedChangeRequest"
				);

				// Query peer log
				let (entries, has_more) = peer_sync
					.get_shared_changes(since_hlc, limit)
					.await
					.map_err(|e| {
						NetworkingError::Protocol(format!("Failed to query shared changes: {}", e))
					})?;

				info!(
					count = entries.len(),
					has_more = has_more,
					"Returning shared changes to requester"
				);

				Ok(Some(SyncMessage::SharedChangeResponse {
					library_id,
					entries,
					current_state: None, // TODO: Add fallback for pruned logs
					has_more,
				}))
			}

			SyncMessage::SharedChangeResponse { .. } => {
				// Response messages are handled separately
				Ok(None)
			}

			SyncMessage::AckSharedChanges {
				library_id,
				from_device,
				up_to_hlc,
			} => {
				peer_sync
					.on_ack_received(from_device, up_to_hlc)
					.await
					.map_err(|e| {
						NetworkingError::Protocol(format!("Failed to process ACK: {}", e))
					})?;

				Ok(None)
			}

			SyncMessage::Heartbeat {
				library_id,
				device_id,
				timestamp,
				state_watermark,
				shared_watermark,
			} => {
				debug!(
					from_device = %from_device,
					device_id = %device_id,
					peer_state_watermark = ?state_watermark,
					peer_shared_watermark = ?shared_watermark,
					"Received heartbeat"
				);

				// Get our current watermarks
				let (our_state_watermark, our_shared_watermark) = peer_sync.get_watermarks().await;

				// Send heartbeat response with our current watermarks
				Ok(Some(SyncMessage::Heartbeat {
					library_id: self.library_id,
					device_id: peer_sync.device_id(),
					timestamp: chrono::Utc::now(),
					state_watermark: our_state_watermark,
					shared_watermark: our_shared_watermark,
				}))
			}

			SyncMessage::Error {
				library_id,
				message,
			} => {
				warn!(
					from_device = %from_device,
					error_message = %message,
					"Received error message from peer"
				);
				Ok(None)
			}
		}
	}
}

#[async_trait]
impl crate::service::network::protocol::ProtocolHandler for SyncProtocolHandler {
	fn protocol_name(&self) -> &'static str {
		"sync"
	}

	async fn handle_stream(
		&self,
		_send: Box<dyn tokio::io::AsyncWrite + Send + Unpin>,
		_recv: Box<dyn tokio::io::AsyncRead + Send + Unpin>,
		_remote_node_id: iroh::NodeId,
	) {
		warn!("SyncProtocolHandler::handle_stream called - not used in request/response model");
	}

	async fn handle_request(&self, from_device: Uuid, request: Vec<u8>) -> Result<Vec<u8>> {
		let message: SyncMessage =
			serde_json::from_slice(&request).map_err(|e| NetworkingError::Serialization(e))?;

		debug!(
			from_device = %from_device,
			"Received sync request"
		);

		match self.handle_sync_message(from_device, message).await? {
			Some(response) => {
				serde_json::to_vec(&response).map_err(|e| NetworkingError::Serialization(e))
			}
			None => Ok(Vec::new()), // No response needed
		}
	}

	async fn handle_response(
		&self,
		from_device: Uuid,
		_from_node: iroh::NodeId,
		response: Vec<u8>,
	) -> Result<()> {
		if response.is_empty() {
			return Ok(());
		}

		let message: SyncMessage =
			serde_json::from_slice(&response).map_err(|e| NetworkingError::Serialization(e))?;

		debug!(
			from_device = %from_device,
			"Received sync response"
		);

		// Process response message
		self.handle_sync_message(from_device, message).await?;

		Ok(())
	}

	async fn handle_event(
		&self,
		_event: crate::service::network::protocol::ProtocolEvent,
	) -> std::result::Result<(), crate::service::network::NetworkingError> {
		// No special event handling needed
		Ok(())
	}

	fn as_any(&self) -> &dyn std::any::Any {
		self
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::service::network::protocol::ProtocolHandler;

	#[test]
	fn test_handler_creation() {
		let handler = SyncProtocolHandler::new(Uuid::new_v4());
		assert_eq!(handler.protocol_name(), "sync");
	}
}
