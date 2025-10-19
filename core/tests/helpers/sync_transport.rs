//! Mock network transport for sync integration tests

use sd_core::{
	infra::sync::{NetworkTransport, Syncable},
	service::network::protocol::sync::messages::SyncMessage,
};
use std::{collections::HashMap, sync::Arc};
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
}

impl MockTransport {
	/// Create a new mock transport for a device
	pub fn new(
		my_device_id: Uuid,
		connected_peers: Vec<Uuid>,
		queues: Arc<Mutex<HashMap<Uuid, Vec<(Uuid, SyncMessage)>>>>,
		history: Arc<Mutex<Vec<(Uuid, Uuid, SyncMessage)>>>,
	) -> Arc<Self> {
		Arc::new(Self {
			my_device_id,
			connected_peers,
			queues,
			history,
		})
	}

	/// Create a pair of connected transports for two devices
	pub fn new_pair(device_a: Uuid, device_b: Uuid) -> (Arc<Self>, Arc<Self>) {
		let queues = Arc::new(Mutex::new(HashMap::new()));
		let history = Arc::new(Mutex::new(Vec::new()));

		let transport_a = Self::new(device_a, vec![device_b], queues.clone(), history.clone());
		let transport_b = Self::new(device_b, vec![device_a], queues.clone(), history.clone());

		(transport_a, transport_b)
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
						.on_state_change_received(sd_core::service::sync::state::StateChangeMessage {
							model_type,
							record_uuid,
							device_id,
							data,
							timestamp,
						})
						.await?;
				}
				SyncMessage::SharedChange { library_id: _, entry } => {
					sync_service.peer_sync().on_shared_change_received(entry).await?;
				}
				SyncMessage::AckSharedChanges {
					library_id: _,
					from_device,
					up_to_hlc,
				} => {
					sync_service.peer_sync().on_ack_received(from_device, up_to_hlc).await?;
				}
				SyncMessage::SharedChangeRequest {
					library_id,
					since_hlc,
					limit,
				} => {
					let (entries, has_more) = sync_service.peer_sync().get_shared_changes(since_hlc, limit).await?;
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
				SyncMessage::SharedChangeResponse { entries, current_state, .. } => {
					// Deliver to backfill manager (it handles duplicate/unexpected responses gracefully)
					let _ = sync_service.backfill_manager().deliver_shared_response(message_clone).await;

					// Also process entries directly (for tests that send manual requests)
					for entry in entries {
						sync_service.peer_sync().on_shared_change_received(entry).await?;
					}

					// Apply current_state snapshot if provided
					if let Some(state) = current_state {
						if let Some(tags) = state["tag"].as_array() {
							for tag_data in tags {
								let uuid: Uuid = Uuid::parse_str(tag_data["uuid"].as_str().unwrap())?;
								let data = tag_data["data"].clone();

								use sd_core::infra::{db::entities, sync::{ChangeType, SharedChangeEntry, HLC}};
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
					let (our_state_watermark, our_shared_watermark) = sync_service.peer_sync().get_watermarks().await;

					let needs_state_catchup = matches!((peer_state_watermark, our_state_watermark), (Some(p), Some(o)) if o > p) || matches!((peer_state_watermark, our_state_watermark), (None, Some(_)));
					let needs_shared_catchup = matches!((peer_shared_watermark, our_shared_watermark), (Some(p), Some(o)) if o > p) || matches!((peer_shared_watermark, our_shared_watermark), (None, Some(_)));

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
						has_more: false,
						checkpoint: None,
					};

					self.send_sync_message(sender, response).await?;
				}
				SyncMessage::StateResponse { .. } => {
					sync_service.backfill_manager().deliver_state_response(message_clone).await?;
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
}

#[async_trait::async_trait]
impl NetworkTransport for MockTransport {
	async fn send_sync_message(&self, target_device: Uuid, message: SyncMessage) -> anyhow::Result<()> {
		if !self.connected_peers.contains(&target_device) {
			return Err(anyhow::anyhow!("device {} not connected", target_device));
		}

		let mut queues = self.queues.lock().await;
		queues
			.entry(target_device)
			.or_insert_with(Vec::new)
			.push((self.my_device_id, message.clone()));
		drop(queues);

		self.history
			.lock()
			.await
			.push((self.my_device_id, target_device, message));

		Ok(())
	}

	async fn get_connected_sync_partners(&self) -> anyhow::Result<Vec<Uuid>> {
		Ok(self.connected_peers.clone())
	}

	async fn is_device_reachable(&self, device_uuid: Uuid) -> bool {
		self.connected_peers.contains(&device_uuid)
	}

	fn transport_name(&self) -> &'static str {
		"MockTransport"
	}
}
