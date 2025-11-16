//! Mock network transport for sync integration tests

use sd_core::{
	infra::sync::{NetworkTransport, Syncable},
	service::{network::protocol::sync::messages::SyncMessage, sync::SyncService},
};
use std::{
	collections::HashMap,
	sync::{Arc, Weak},
};
use tokio::sync::Mutex;
use uuid::Uuid;

/// Unified mock transport for N-device sync tests
pub struct MockTransport {
	my_device_id: Uuid,
	connected_peers: Vec<Uuid>,
	/// Shared message queues: recipient_id -> messages for them
	queues: Arc<Mutex<HashMap<Uuid, Vec<(Uuid, SyncMessage)>>>>,
	/// Complete message history: (from, to, message)
	history: Arc<Mutex<Vec<(Uuid, Uuid, SyncMessage)>>>,
	/// Shared sync service registry for request/response handling
	sync_services: Arc<Mutex<HashMap<Uuid, Weak<SyncService>>>>,
}

impl MockTransport {
	/// Create a new mock transport for a device
	pub fn new(
		my_device_id: Uuid,
		connected_peers: Vec<Uuid>,
		queues: Arc<Mutex<HashMap<Uuid, Vec<(Uuid, SyncMessage)>>>>,
		history: Arc<Mutex<Vec<(Uuid, Uuid, SyncMessage)>>>,
		sync_services: Arc<Mutex<HashMap<Uuid, Weak<SyncService>>>>,
	) -> Arc<Self> {
		Arc::new(Self {
			my_device_id,
			connected_peers,
			queues,
			history,
			sync_services,
		})
	}

	/// Create a pair of connected transports for two devices
	pub fn new_pair(device_a: Uuid, device_b: Uuid) -> (Arc<Self>, Arc<Self>) {
		let queues = Arc::new(Mutex::new(HashMap::new()));
		let history = Arc::new(Mutex::new(Vec::new()));
		let sync_services = Arc::new(Mutex::new(HashMap::new()));

		let transport_a = Self::new(
			device_a,
			vec![device_b],
			queues.clone(),
			history.clone(),
			sync_services.clone(),
		);
		let transport_b = Self::new(
			device_b,
			vec![device_a],
			queues.clone(),
			history.clone(),
			sync_services.clone(),
		);

		(transport_a, transport_b)
	}

	/// Register a sync service for request/response handling
	pub async fn register_sync_service(&self, device_id: Uuid, sync_service: Weak<SyncService>) {
		self.sync_services
			.lock()
			.await
			.insert(device_id, sync_service);
	}

	/// Process incoming messages by delivering them to the sync service
	pub async fn process_incoming_messages(
		&self,
		sync_service: &sd_core::service::sync::SyncService,
	) -> anyhow::Result<usize> {
		let mut queues = self.queues.lock().await;
		let messages = queues.entry(self.my_device_id).or_insert_with(Vec::new);
		let incoming: Vec<_> = messages.drain(..).collect();
		let count = incoming.len();
		drop(queues);

		for (sender, message) in incoming {
			let message_clone = message.clone();

			match message {
				SyncMessage::StateChange {
					library_id: _,
					model_type,
					record_uuid,
					device_id,
					data,
					timestamp,
				} => {
					sync_service
						.peer_sync()
						.on_state_change_received(
							sd_core::service::sync::state::StateChangeMessage {
								model_type,
								record_uuid,
								device_id,
								data,
								timestamp,
							},
						)
						.await?;
				}
				SyncMessage::SharedChange {
					library_id: _,
					entry,
				} => {
					sync_service
						.peer_sync()
						.on_shared_change_received(entry)
						.await?;
				}
				SyncMessage::AckSharedChanges {
					library_id: _,
					from_device,
					up_to_hlc,
				} => {
					sync_service
						.peer_sync()
						.on_ack_received(from_device, up_to_hlc)
						.await?;
				}
				SyncMessage::SharedChangeRequest {
					library_id,
					since_hlc,
					limit,
				} => {
					let (entries, has_more) = sync_service
						.peer_sync()
						.get_shared_changes(since_hlc, limit)
						.await?;
					let current_state = if since_hlc.is_none() {
						Some(sync_service.peer_sync().get_full_shared_state().await?)
					} else {
						None
					};

					let response = SyncMessage::SharedChangeResponse {
						library_id,
						entries,
						current_state,
						has_more,
					};

					self.send_sync_message(sender, response).await?;
				}
				SyncMessage::SharedChangeResponse {
					entries,
					current_state,
					..
				} => {
					// Deliver to backfill manager (it handles duplicate/unexpected responses gracefully)
					let _ = sync_service
						.backfill_manager()
						.deliver_shared_response(message_clone)
						.await;

					// Also process entries directly (for tests that send manual requests)
					for entry in entries {
						sync_service
							.peer_sync()
							.on_shared_change_received(entry)
							.await?;
					}

					// Apply current_state snapshot if provided
					if let Some(state) = current_state {
						if let Some(tags) = state["tag"].as_array() {
							for tag_data in tags {
								let uuid: Uuid =
									Uuid::parse_str(tag_data["uuid"].as_str().unwrap())?;
								let data = tag_data["data"].clone();

								use sd_core::infra::{
									db::entities,
									sync::{ChangeType, SharedChangeEntry, HLC},
								};
								entities::tag::Model::apply_shared_change(
									SharedChangeEntry {
										hlc: HLC::now(self.my_device_id),
										model_type: "tag".to_string(),
										record_uuid: uuid,
										change_type: ChangeType::Insert,
										data,
									},
									sync_service.peer_sync().db().as_ref(),
								)
								.await?;
							}
						}
					}
				}
				SyncMessage::WatermarkExchangeRequest {
					library_id,
					device_id: requesting_device_id,
					my_state_watermark: peer_state_watermark,
					my_shared_watermark: peer_shared_watermark,
				} => {
					let (our_state_watermark, our_shared_watermark) =
						sync_service.peer_sync().get_watermarks().await;

					let needs_state_catchup = matches!((peer_state_watermark, our_state_watermark), (Some(p), Some(o)) if o > p)
						|| matches!((peer_state_watermark, our_state_watermark), (None, Some(_)));
					let needs_shared_catchup = matches!((peer_shared_watermark, our_shared_watermark), (Some(p), Some(o)) if o > p)
						|| matches!(
							(peer_shared_watermark, our_shared_watermark),
							(None, Some(_))
						);

					let response = SyncMessage::WatermarkExchangeResponse {
						library_id,
						device_id: self.my_device_id,
						state_watermark: our_state_watermark,
						shared_watermark: our_shared_watermark,
						needs_state_catchup,
						needs_shared_catchup,
					};

					self.send_sync_message(sender, response).await?;
				}
				SyncMessage::WatermarkExchangeResponse {
					library_id: _,
					device_id: peer_device_id,
					state_watermark: peer_state_watermark,
					shared_watermark: peer_shared_watermark,
					needs_state_catchup,
					needs_shared_catchup,
				} => {
					sync_service
						.peer_sync()
						.on_watermark_exchange_response(
							peer_device_id,
							peer_state_watermark,
							peer_shared_watermark,
							needs_state_catchup,
							needs_shared_catchup,
						)
						.await?;
				}
				SyncMessage::StateRequest {
					library_id,
					model_types,
					device_id: requested_device_id,
					since: _,
					checkpoint: _,
					batch_size: _,
				} => {
					let response = SyncMessage::StateResponse {
						library_id,
						model_type: model_types.first().cloned().unwrap_or_default(),
						device_id: requested_device_id.unwrap_or(self.my_device_id),
						records: vec![],
						deleted_uuids: vec![],
						has_more: false,
						checkpoint: None,
					};

					self.send_sync_message(sender, response).await?;
				}
				SyncMessage::StateResponse { .. } => {
					sync_service
						.backfill_manager()
						.deliver_state_response(message_clone)
						.await?;
				}
				_ => {}
			}
		}

		Ok(count)
	}

	/// Get all messages sent from one device to another
	pub async fn get_messages_between(&self, from: Uuid, to: Uuid) -> Vec<SyncMessage> {
		self.history
			.lock()
			.await
			.iter()
			.filter(|(f, t, _)| *f == from && *t == to)
			.map(|(_, _, msg)| msg.clone())
			.collect()
	}

	/// Get all messages sent by a device
	pub async fn get_messages_from(&self, from: Uuid) -> Vec<(Uuid, SyncMessage)> {
		self.history
			.lock()
			.await
			.iter()
			.filter(|(f, _, _)| *f == from)
			.map(|(_, t, msg)| (*t, msg.clone()))
			.collect()
	}

	/// Get total message count in history
	pub async fn total_message_count(&self) -> usize {
		self.history.lock().await.len()
	}

	/// Get queue size for a device
	pub async fn queue_size(&self, device_id: Uuid) -> usize {
		self.queues
			.lock()
			.await
			.get(&device_id)
			.map(|q| q.len())
			.unwrap_or(0)
	}

	/// Deliver a single message to a sync service (simulates production handle_sync_message)
	async fn deliver_message(
		sync_service: &sd_core::service::sync::SyncService,
		_sender: Uuid,
		message: SyncMessage,
	) -> anyhow::Result<()> {
		use sd_core::service::sync::state::StateChangeMessage;

		match message {
			SyncMessage::StateChange {
				library_id: _,
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
				sync_service
					.peer_sync()
					.on_state_change_received(change)
					.await?;
			}
			SyncMessage::SharedChange {
				library_id: _,
				entry,
			} => {
				sync_service
					.peer_sync()
					.on_shared_change_received(entry)
					.await?;
			}
			_ => {
				// Other message types handled differently
			}
		}
		Ok(())
	}
}

#[async_trait::async_trait]
impl NetworkTransport for MockTransport {
	async fn send_sync_message(
		&self,
		target_device: Uuid,
		message: SyncMessage,
	) -> anyhow::Result<()> {
		if !self.connected_peers.contains(&target_device) {
			return Err(anyhow::anyhow!("device {} not connected", target_device));
		}

		// Record in history
		self.history
			.lock()
			.await
			.push((self.my_device_id, target_device, message.clone()));

		// In production, handle_sync_message is called synchronously (no spawn)
		// It's already within an async context (the network stream handler)
		// We should do the same - deliver immediately in this async fn

		tracing::trace!(
			from = %self.my_device_id,
			to = %target_device,
			message_type = ?std::mem::discriminant(&message),
			"[MockTransport] send_sync_message called, delivering immediately"
		);

		// Get target's sync service
		let sync_service = {
			let services = self.sync_services.lock().await;
			services
				.get(&target_device)
				.and_then(|weak| weak.upgrade())
				.ok_or_else(|| {
					tracing::warn!(
						target = %target_device,
						"[MockTransport] Target sync service not registered"
					);
					anyhow::anyhow!(
						"Target sync service not registered for device {}",
						target_device
					)
				})?
		};

		// Deliver immediately (simulates production's synchronous handle_sync_message call)
		tracing::debug!(
			from = %self.my_device_id,
			to = %target_device,
			"[MockTransport] Delivering message to target sync service"
		);

		MockTransport::deliver_message(&sync_service, self.my_device_id, message).await?;

		tracing::debug!(
			from = %self.my_device_id,
			to = %target_device,
			"[MockTransport] Message delivered successfully"
		);

		Ok(())
	}

	async fn send_sync_request(
		&self,
		target_device: Uuid,
		request: SyncMessage,
	) -> anyhow::Result<SyncMessage> {
		// For testing: invoke the actual protocol handler on the target device
		// This simulates the bidirectional stream request/response pattern

		if !self.connected_peers.contains(&target_device) {
			return Err(anyhow::anyhow!("device {} not connected", target_device));
		}

		// Get the target device's sync service
		let sync_service = {
			let services = self.sync_services.lock().await;
			services
				.get(&target_device)
				.and_then(|weak| weak.upgrade())
				.ok_or_else(|| {
					anyhow::anyhow!(
						"Target sync service not registered for device {}",
						target_device
					)
				})?
		};

		// Record in history
		self.history
			.lock()
			.await
			.push((self.my_device_id, target_device, request.clone()));

		// Process the request through the target's protocol handler to get real response
		let response = match &request {
			SyncMessage::StateRequest {
				model_types,
				device_id,
				since,
				checkpoint,
				batch_size,
				..
			} => {
				// Parse checkpoint cursor
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

				// Query actual state from target device's database
				let records = sync_service
					.peer_sync()
					.get_device_state(model_types.clone(), *device_id, *since, cursor, *batch_size)
					.await?;

				// Query tombstones if incremental sync
				let deleted_uuids = if let Some(since_time) = since {
					sync_service
						.peer_sync()
						.get_deletion_tombstones(
							model_types.first().unwrap_or(&String::new()),
							*device_id,
							*since_time,
						)
						.await?
				} else {
					vec![]
				};

				let has_more = records.len() >= *batch_size;
				let next_checkpoint = if has_more {
					records
						.last()
						.map(|r| format!("{}|{}", r.timestamp.to_rfc3339(), r.uuid))
				} else {
					None
				};

				SyncMessage::StateResponse {
					library_id: request.library_id(),
					model_type: model_types.first().cloned().unwrap_or_default(),
					device_id: device_id.unwrap_or(target_device),
					records,
					deleted_uuids,
					checkpoint: next_checkpoint,
					has_more,
				}
			}
			SyncMessage::SharedChangeRequest {
				since_hlc, limit, ..
			} => {
				// Query actual shared changes from target device
				let (entries, has_more) = sync_service
					.peer_sync()
					.get_shared_changes(*since_hlc, *limit)
					.await?;

				// Include current state snapshot if initial backfill
				let current_state = if since_hlc.is_none() {
					Some(sync_service.peer_sync().get_full_shared_state().await?)
				} else {
					None
				};

				SyncMessage::SharedChangeResponse {
					library_id: request.library_id(),
					entries,
					current_state,
					has_more,
				}
			}
			_ => {
				return Err(anyhow::anyhow!(
					"send_sync_request called with non-request message type"
				));
			}
		};

		Ok(response)
	}

	async fn get_connected_sync_partners(
		&self,
		_library_id: Uuid,
		_db: &sea_orm::DatabaseConnection,
	) -> anyhow::Result<Vec<Uuid>> {
		Ok(self.connected_peers.clone())
	}

	async fn is_device_reachable(&self, device_uuid: Uuid) -> bool {
		self.connected_peers.contains(&device_uuid)
	}

	fn transport_name(&self) -> &'static str {
		"MockTransport"
	}
}
