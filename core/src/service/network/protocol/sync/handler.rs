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
	backfill_manager: Option<Arc<crate::service::sync::BackfillManager>>,
	device_registry: Arc<tokio::sync::RwLock<crate::service::network::device::DeviceRegistry>>,
}

impl SyncProtocolHandler {
	/// Create a new sync protocol handler
	pub fn new(
		library_id: Uuid,
		device_registry: Arc<tokio::sync::RwLock<crate::service::network::device::DeviceRegistry>>,
	) -> Self {
		info!(
			library_id = %library_id,
			"Creating SyncProtocolHandler for leaderless hybrid sync"
		);
		Self {
			library_id,
			peer_sync: None,
			backfill_manager: None,
			device_registry,
		}
	}

	/// Set the peer sync service (called after initialization)
	pub fn set_peer_sync(&mut self, peer_sync: Arc<crate::service::sync::peer::PeerSync>) {
		self.peer_sync = Some(peer_sync);
	}

	/// Set the backfill manager (called after initialization)
	pub fn set_backfill_manager(
		&mut self,
		backfill_manager: Arc<crate::service::sync::BackfillManager>,
	) {
		self.backfill_manager = Some(backfill_manager);
	}

	/// Get library ID
	pub fn library_id(&self) -> Uuid {
		self.library_id
	}

	/// Handle an incoming sync message
	pub async fn handle_sync_message(
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
				info!(
					from_device = %from_device,
					model_type = %model_type,
					record_uuid = %record_uuid,
					"Processing state change"
				);

				let change = StateChangeMessage {
					model_type: model_type.clone(),
					record_uuid,
					device_id,
					data,
					timestamp,
				};

				peer_sync
					.on_state_change_received(change)
					.await
					.map_err(|e| {
						warn!(
							model_type = %model_type,
							error = %e,
							"Failed to apply state change"
						);
						NetworkingError::Protocol(format!("Failed to apply state change: {}", e))
					})?;

				info!(
					model_type = %model_type,
					"State change applied successfully"
				);

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

				// Parse checkpoint to get cursor (timestamp + uuid)
				let cursor = checkpoint.as_ref().and_then(|chk| {
					let parts: Vec<&str> = chk.split('|').collect();
					if parts.len() == 2 {
						let ts = chrono::DateTime::parse_from_rfc3339(parts[0])
							.ok()?
							.with_timezone(&chrono::Utc);
						let uuid = Uuid::parse_str(parts[1]).ok()?;
						Some((ts, uuid))
					} else {
						None
					}
				});

				// Query local state
				let records = peer_sync
					.get_device_state(model_types.clone(), device_id, since, cursor, batch_size)
					.await
					.map_err(|e| {
						NetworkingError::Protocol(format!("Failed to query device state: {}", e))
					})?;

				// Query tombstones if incremental sync
				let model_type = model_types.first().cloned().unwrap_or_default();
				let deleted_uuids = if let Some(since_time) = since {
					peer_sync
						.get_deletion_tombstones(&model_type, device_id, since_time)
						.await
						.map_err(|e| {
							NetworkingError::Protocol(format!("Failed to query tombstones: {}", e))
						})?
				} else {
					vec![] // Full sync doesn't need tombstones
				};

				let has_more = records.len() >= batch_size;

				// Create checkpoint: "timestamp|uuid" format
				let next_checkpoint = if has_more {
					records
						.last()
						.map(|r| format!("{}|{}", r.timestamp.to_rfc3339(), r.uuid))
				} else {
					None
				};

				Ok(Some(SyncMessage::StateResponse {
					library_id,
					model_type,
					device_id: device_id.unwrap_or(from_device),
					records,
					deleted_uuids,
					checkpoint: next_checkpoint,
					has_more,
				}))
			}

			SyncMessage::StateResponse { .. } => {
				// Deliver to backfill manager if available
				if let Some(backfill_manager) = &self.backfill_manager {
					backfill_manager
						.deliver_state_response(message)
						.await
						.map_err(|e| {
							NetworkingError::Protocol(format!(
								"Failed to deliver StateResponse: {}",
								e
							))
						})?;
				} else {
					warn!("Received StateResponse but backfill manager not set");
				}
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

				// If initial backfill (since_hlc = None), include full current state
				let current_state = if since_hlc.is_none() {
					debug!("Initial backfill requested - querying full shared resource state");
					match peer_sync.get_full_shared_state().await {
						Ok(state) => {
							info!("Including full state snapshot for initial backfill");
							Some(state)
						}
						Err(e) => {
							warn!("Failed to query full shared state: {}", e);
							None
						}
					}
				} else {
					None
				};

				info!(
					count = entries.len(),
					has_more = has_more,
					has_state_snapshot = current_state.is_some(),
					"Returning shared changes to requester"
				);

				Ok(Some(SyncMessage::SharedChangeResponse {
					library_id,
					entries,
					current_state,
					has_more,
				}))
			}

			SyncMessage::SharedChangeResponse { .. } => {
				// Deliver to backfill manager if available
				if let Some(backfill_manager) = &self.backfill_manager {
					backfill_manager
						.deliver_shared_response(message)
						.await
						.map_err(|e| {
							NetworkingError::Protocol(format!(
								"Failed to deliver SharedChangeResponse: {}",
								e
							))
						})?;
				} else {
					warn!("Received SharedChangeResponse but backfill manager not set");
				}
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

			SyncMessage::WatermarkExchangeRequest {
				library_id,
				device_id,
				my_shared_watermark: peer_shared_watermark,
				my_resource_watermarks: peer_resource_watermarks,
			} => {
				debug!(
					from_device = %from_device,
					peer_shared_watermark = ?peer_shared_watermark,
					peer_resource_count = peer_resource_watermarks.len(),
					"Processing WatermarkExchangeRequest with per-resource watermarks"
				);

				// Get our current watermarks
				let (_our_state_watermark, our_shared_watermark) = peer_sync.get_watermarks().await;
				let our_resource_watermarks =
					crate::infra::sync::ResourceWatermarkStore::new(peer_sync.device_id())
						.get_our_resource_watermarks(peer_sync.peer_log_conn())
						.await
						.unwrap_or_default();

				// Determine if peer needs catch-up by comparing per-resource watermarks
				let mut needs_state_catchup = false;
				for (resource_type, our_ts) in &our_resource_watermarks {
					match peer_resource_watermarks.get(resource_type) {
						Some(peer_ts) if our_ts > peer_ts => {
							needs_state_catchup = true;
							break;
						}
						None => {
							needs_state_catchup = true;
							break;
						}
						_ => {}
					}
				}

				let needs_shared_catchup = match (peer_shared_watermark, our_shared_watermark) {
					(Some(peer_hlc), Some(our_hlc)) => our_hlc > peer_hlc,
					(None, Some(_)) => true,
					_ => false,
				};

				info!(
					from_device = %from_device,
					needs_state_catchup = needs_state_catchup,
					needs_shared_catchup = needs_shared_catchup,
					our_resource_count = our_resource_watermarks.len(),
					"Responding to watermark exchange request with per-resource watermarks"
				);

				Ok(Some(SyncMessage::WatermarkExchangeResponse {
					library_id: self.library_id,
					device_id: peer_sync.device_id(),
					shared_watermark: our_shared_watermark,
					needs_state_catchup,
					needs_shared_catchup,
					resource_watermarks: our_resource_watermarks,
				}))
			}

			SyncMessage::WatermarkExchangeResponse {
				library_id,
				device_id,
				shared_watermark: peer_shared_watermark,
				needs_state_catchup,
				needs_shared_catchup,
				resource_watermarks: peer_resource_watermarks,
			} => {
				debug!(
					from_device = %from_device,
					peer_shared_watermark = ?peer_shared_watermark,
					needs_state_catchup = needs_state_catchup,
					needs_shared_catchup = needs_shared_catchup,
					peer_resource_count = peer_resource_watermarks.len(),
					"Processing WatermarkExchangeResponse with per-resource watermarks"
				);

				peer_sync
					.on_watermark_exchange_response(
						from_device,
						peer_shared_watermark,
						needs_state_catchup,
						needs_shared_catchup,
						peer_resource_watermarks,
					)
					.await
					.map_err(|e| {
						NetworkingError::Protocol(format!(
							"Failed to handle watermark exchange response: {}",
							e
						))
					})?;

				Ok(None)
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

	fn as_any(&self) -> &dyn std::any::Any {
		self
	}

	async fn handle_stream(
		&self,
		mut send: Box<dyn tokio::io::AsyncWrite + Send + Unpin>,
		mut recv: Box<dyn tokio::io::AsyncRead + Send + Unpin>,
		remote_node_id: iroh::NodeId,
	) {
		use tokio::io::{AsyncReadExt, AsyncWriteExt};

		tracing::info!(
			"SyncProtocolHandler: Stream accepted from node {}",
			remote_node_id
		);

		// Map node_id to device_id using device registry
		let from_device = {
			let registry = self.device_registry.read().await;
			registry.get_device_by_node(remote_node_id)
		};

		let from_device = match from_device {
			Some(id) => {
				tracing::info!("SyncProtocolHandler: Mapped to device_id {}", id);
				id
			}
			None => {
				tracing::warn!(
					"SyncProtocolHandler: Received sync stream from unknown node {}, closing gracefully",
					remote_node_id
				);
				return;
			}
		};

		// Read request with length prefix
		tracing::info!(
			"SyncProtocolHandler: Reading request from device {}...",
			from_device
		);
		let mut len_buf = [0u8; 4];
		if let Err(e) = recv.read_exact(&mut len_buf).await {
			// This is normal if peer just opened connection to test connectivity
			tracing::debug!(
				"SyncProtocolHandler: Failed to read sync request length (likely connection test): {}",
				e
			);
			return;
		}
		let req_len = u32::from_be_bytes(len_buf) as usize;

		let mut req_buf = vec![0u8; req_len];
		if let Err(e) = recv.read_exact(&mut req_buf).await {
			tracing::error!("Failed to read sync request: {}", e);
			return;
		}

		// Deserialize request
		let request: SyncMessage = match serde_json::from_slice(&req_buf) {
			Ok(msg) => msg,
			Err(e) => {
				tracing::error!("Failed to deserialize sync request: {}", e);
				return;
			}
		};

		tracing::debug!(
			from_device = %from_device,
			message_type = ?std::mem::discriminant(&request),
			"Processing sync request via bidirectional stream"
		);

		// Handle the request and get response
		let response_opt = match self.handle_sync_message(from_device, request).await {
			Ok(resp) => resp,
			Err(e) => {
				tracing::error!("Failed to handle sync message: {}", e);
				return;
			}
		};

		// Send response if handler returned one
		if let Some(response) = response_opt {
			let resp_bytes = match serde_json::to_vec(&response) {
				Ok(bytes) => bytes,
				Err(e) => {
					tracing::error!("Failed to serialize sync response: {}", e);
					return;
				}
			};

			let len = resp_bytes.len() as u32;
			if let Err(e) = send.write_all(&len.to_be_bytes()).await {
				tracing::error!("Failed to send response length: {}", e);
				return;
			}
			if let Err(e) = send.write_all(&resp_bytes).await {
				tracing::error!("Failed to send response: {}", e);
				return;
			}
			let _ = send.flush().await;

			tracing::debug!(
				from_device = %from_device,
				response_bytes = resp_bytes.len(),
				"Sent sync response"
			);
		}
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
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::service::network::protocol::ProtocolHandler;

	#[test]
	fn test_handler_creation() {
		// Test uses mock registry
		use crate::device::DeviceManager;
		use crate::service::network::device::DeviceRegistry;
		use std::path::PathBuf;

		let device_manager = Arc::new(DeviceManager::new().unwrap());
		let logger = Arc::new(crate::service::network::utils::SilentLogger);
		let registry =
			DeviceRegistry::new(device_manager, PathBuf::from("/tmp/test"), logger).unwrap();
		let device_registry = Arc::new(tokio::sync::RwLock::new(registry));

		let handler = SyncProtocolHandler::new(Uuid::new_v4(), device_registry);
		assert_eq!(handler.protocol_name(), "sync");
	}
}
