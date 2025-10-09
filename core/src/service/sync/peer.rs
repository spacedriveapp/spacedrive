//! Peer sync service - Leaderless architecture
//!
//! All devices are peers, using hybrid sync:
//! - State-based for device-owned data
//! - Log-based with HLC for shared resources

use crate::{
	infra::{
		event::{Event, EventBus},
		sync::{HLCGenerator, NetworkTransport, PeerLog, PeerLogError, SharedChangeEntry, HLC},
	},
	library::Library,
	service::network::protocol::sync::messages::SyncMessage,
};
use anyhow::Result;
use chrono::Utc;
use sea_orm::DatabaseConnection;
use std::sync::{
	atomic::{AtomicBool, Ordering},
	Arc,
};
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use super::{
	retry_queue::RetryQueue,
	state::{BufferQueue, DeviceSyncState, StateChangeMessage},
};

/// Peer sync service for leaderless architecture
///
/// Handles both state-based (device-owned) and log-based (shared) sync.
pub struct PeerSync {
	/// Library ID
	library_id: Uuid,

	/// This device's ID
	device_id: Uuid,

	/// Database connection
	db: Arc<DatabaseConnection>,

	/// Network transport for sending sync messages
	network: Arc<dyn NetworkTransport>,

	/// Sync state machine
	pub(super) state: Arc<RwLock<DeviceSyncState>>,

	/// Buffer for updates during backfill/catch-up
	buffer: Arc<BufferQueue>,

	/// HLC generator for this device
	hlc_generator: Arc<tokio::sync::Mutex<HLCGenerator>>,

	/// Per-peer sync log
	pub(super) peer_log: Arc<PeerLog>,

	/// Event bus
	event_bus: Arc<EventBus>,

	/// Retry queue for failed messages
	retry_queue: Arc<RetryQueue>,

	/// Whether the service is running
	is_running: Arc<AtomicBool>,
}

impl PeerSync {
	/// Create new peer sync service
	pub async fn new(
		library: &Library,
		device_id: Uuid,
		peer_log: Arc<PeerLog>,
		network: Arc<dyn NetworkTransport>,
	) -> Result<Self> {
		let library_id = library.id();

		info!(
			library_id = %library_id,
			device_id = %device_id,
			"Creating peer sync service"
		);

		Ok(Self {
			library_id,
			device_id,
			db: Arc::new(library.db().conn().clone()),
			network,
			state: Arc::new(RwLock::new(DeviceSyncState::Uninitialized)),
			buffer: Arc::new(BufferQueue::new()),
			hlc_generator: Arc::new(tokio::sync::Mutex::new(HLCGenerator::new(device_id))),
			peer_log,
			event_bus: library.event_bus().clone(),
			retry_queue: Arc::new(RetryQueue::new()),
			is_running: Arc::new(AtomicBool::new(false)),
		})
	}

	/// Get database connection
	pub fn db(&self) -> &Arc<DatabaseConnection> {
		&self.db
	}

	/// Get this device's ID
	pub fn device_id(&self) -> Uuid {
		self.device_id
	}

	/// Get watermarks for heartbeat
	pub async fn get_watermarks(&self) -> (Option<chrono::DateTime<chrono::Utc>>, Option<HLC>) {
		// State watermark: Would need to track last state change timestamp
		// For now, return None - this would require adding timestamp tracking
		let state_watermark = None;

		// Shared watermark: Get last HLC from generator
		let shared_watermark = self.hlc_generator.lock().await.last();

		(state_watermark, shared_watermark)
	}

	/// Start the sync service
	pub async fn start(&self) -> Result<()> {
		if self.is_running.load(Ordering::SeqCst) {
			warn!("Peer sync service already running");
			return Ok(());
		}

		info!(
			library_id = %self.library_id,
			device_id = %self.device_id,
			"Starting peer sync service"
		);

		self.is_running.store(true, Ordering::SeqCst);

		// Start event listener for TransactionManager events
		self.start_event_listener();

		// Start background task for retry queue processing
		self.start_retry_processor();

		// Start background task for periodic log pruning
		self.start_log_pruner();

		Ok(())
	}

	/// Start background task to process retry queue
	fn start_retry_processor(&self) {
		let retry_queue = self.retry_queue.clone();
		let network = self.network.clone();
		let is_running = self.is_running.clone();

		tokio::spawn(async move {
			info!("Started retry queue processor");

			while is_running.load(Ordering::SeqCst) {
				// Check for ready messages every 10 seconds
				tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;

				// Get messages ready for retry
				let ready_messages = retry_queue.get_ready().await;

				if !ready_messages.is_empty() {
					debug!(count = ready_messages.len(), "Processing retry queue");
				}

				// Attempt to send each message
				for (target_device, message) in ready_messages {
					match network
						.send_sync_message(target_device, message.clone())
						.await
					{
						Ok(()) => {
							debug!(target = %target_device, "Retry successful");
						}
						Err(e) => {
							warn!(
								target = %target_device,
								error = %e,
								"Retry failed, will retry again"
							);
							// Message will be re-queued automatically by get_ready()
						}
					}
				}
			}

			info!("Retry queue processor stopped");
		});
	}

	/// Start background task for periodic log pruning
	fn start_log_pruner(&self) {
		let peer_log = self.peer_log.clone();
		let is_running = self.is_running.clone();

		tokio::spawn(async move {
			info!("Started log pruner");

			while is_running.load(Ordering::SeqCst) {
				// Prune every 5 minutes
				tokio::time::sleep(tokio::time::Duration::from_secs(300)).await;

				match peer_log.prune_acked().await {
					Ok(count) if count > 0 => {
						info!(pruned = count, "Pruned acknowledged log entries");
					}
					Ok(_) => {
						debug!("No log entries to prune");
					}
					Err(e) => {
						warn!(error = %e, "Failed to prune log");
					}
				}
			}

			info!("Log pruner stopped");
		});
	}

	/// Start event listener for TransactionManager sync events
	fn start_event_listener(&self) {
		// Clone necessary fields for the spawned task
		let library_id = self.library_id;
		let network = self.network.clone();
		info!(
			"PeerSync event listener cloning network transport: {:?}",
			std::any::type_name_of_val(&*network)
		);
		let state = self.state.clone();
		let buffer = self.buffer.clone();
		let db = self.db.clone();
		let event_bus_for_emit = self.event_bus.clone();
		let retry_queue = self.retry_queue.clone();
		let mut subscriber = self.event_bus.subscribe();
		let is_running = self.is_running.clone();

		tokio::spawn(async move {
			info!(
				"PeerSync event listener started with network transport: {}",
				network.transport_name()
			);

			while is_running.load(Ordering::SeqCst) {
				match subscriber.recv().await {
					Ok(Event::Custom { event_type, data }) => {
						match event_type.as_str() {
							"sync:state_change" => {
								if let Err(e) = Self::handle_state_change_event_static(
									library_id,
									data,
									&network,
									&state,
									&buffer,
									&retry_queue,
								)
								.await
								{
									warn!(error = %e, "Failed to handle state change event");
								}
							}
							"sync:shared_change" => {
								if let Err(e) = Self::handle_shared_change_event_static(
									library_id,
									data,
									&network,
									&state,
									&buffer,
									&retry_queue,
								)
								.await
								{
									warn!(error = %e, "Failed to handle shared change event");
								}
							}
							_ => {
								// Ignore other custom events
							}
						}
					}
					Ok(_) => {
						// Ignore non-custom events
					}
					Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
						warn!(
							skipped = skipped,
							"Event listener lagged, some events skipped"
						);
					}
					Err(tokio::sync::broadcast::error::RecvError::Closed) => {
						info!("Event bus closed, stopping event listener");
						break;
					}
				}
			}

			info!("PeerSync event listener stopped");
		});
	}

	/// Handle state change event from TransactionManager (static version for spawned task)
	async fn handle_state_change_event_static(
		library_id: Uuid,
		data: serde_json::Value,
		network: &Arc<dyn NetworkTransport>,
		state: &Arc<RwLock<DeviceSyncState>>,
		buffer: &Arc<BufferQueue>,
		retry_queue: &Arc<RetryQueue>,
	) -> Result<()> {
		let model_type: String = data
			.get("model_type")
			.and_then(|v| v.as_str())
			.ok_or_else(|| anyhow::anyhow!("Missing model_type in state_change event"))?
			.to_string();

		let record_uuid: Uuid = data
			.get("record_uuid")
			.and_then(|v| v.as_str())
			.and_then(|s| Uuid::parse_str(s).ok())
			.ok_or_else(|| anyhow::anyhow!("Missing or invalid record_uuid"))?;

		let device_id: Uuid = data
			.get("device_id")
			.and_then(|v| v.as_str())
			.and_then(|s| Uuid::parse_str(s).ok())
			.ok_or_else(|| anyhow::anyhow!("Missing or invalid device_id"))?;

		let data_value = data
			.get("data")
			.ok_or_else(|| anyhow::anyhow!("Missing data in state_change event"))?
			.clone();

		let timestamp = data
			.get("timestamp")
			.and_then(|v| v.as_str())
			.and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
			.map(|dt| dt.with_timezone(&chrono::Utc))
			.unwrap_or_else(Utc::now);

		let change = StateChangeMessage {
			model_type,
			record_uuid,
			device_id,
			data: data_value,
			timestamp,
		};

		debug!(
			model_type = %change.model_type,
			record_uuid = %change.record_uuid,
			"Broadcasting state change from event"
		);

		// Check if we should buffer
		let current_state = *state.read().await;
		if current_state.should_buffer() {
			debug!("Buffering own state change during backfill");
			buffer
				.push(super::state::BufferedUpdate::StateChange(change))
				.await;
			return Ok(());
		}

		// Get all connected sync partners
		debug!("About to call network.get_connected_sync_partners() on handle_state_change_event_static");
		let connected_partners = network.get_connected_sync_partners().await.map_err(|e| {
			warn!(error = %e, "Failed to get connected partners");
			e
		})?;

		debug!(
			count = connected_partners.len(),
			partners = ?connected_partners,
			"[Static Handler] Got connected sync partners from transport"
		);

		if connected_partners.is_empty() {
			debug!("[Static Handler] No connected sync partners to broadcast to");
			return Ok(());
		}

		// Create sync message
		let message = SyncMessage::StateChange {
			library_id,
			model_type: change.model_type.clone(),
			record_uuid: change.record_uuid,
			device_id: change.device_id,
			data: change.data.clone(),
			timestamp: Utc::now(),
		};

		debug!(
			model_type = %change.model_type,
			record_uuid = %change.record_uuid,
			partner_count = connected_partners.len(),
			"Broadcasting state change to sync partners"
		);

		// Broadcast to all partners in parallel
		use futures::future::join_all;

		let send_futures: Vec<_> = connected_partners
			.iter()
			.map(|&partner| {
				let network = network.clone();
				let msg = message.clone();
				async move {
					match tokio::time::timeout(
						std::time::Duration::from_secs(30),
						network.send_sync_message(partner, msg),
					)
					.await
					{
						Ok(Ok(())) => (partner, Ok(())),
						Ok(Err(e)) => (partner, Err(e)),
						Err(_) => (partner, Err(anyhow::anyhow!("Send timeout after 30s"))),
					}
				}
			})
			.collect();

		let results = join_all(send_futures).await;

		// Process results
		let mut success_count = 0;
		let mut error_count = 0;

		for (partner_uuid, result) in results {
			match result {
				Ok(()) => {
					success_count += 1;
					debug!(partner = %partner_uuid, "State change sent successfully");
				}
				Err(e) => {
					error_count += 1;
					warn!(
						partner = %partner_uuid,
						error = %e,
						"Failed to send state change to partner, enqueuing for retry"
					);
					// Enqueue for retry
					retry_queue.enqueue(partner_uuid, message.clone()).await;
				}
			}
		}

		info!(
			model_type = %change.model_type,
			success = success_count,
			errors = error_count,
			"State change broadcast complete"
		);

		Ok(())
	}

	/// Handle shared change event from TransactionManager (static version for spawned task)
	async fn handle_shared_change_event_static(
		library_id: Uuid,
		data: serde_json::Value,
		network: &Arc<dyn NetworkTransport>,
		state: &Arc<RwLock<DeviceSyncState>>,
		buffer: &Arc<BufferQueue>,
		retry_queue: &Arc<RetryQueue>,
	) -> Result<()> {
		let entry: SharedChangeEntry = serde_json::from_value(
			data.get("entry")
				.ok_or_else(|| anyhow::anyhow!("Missing entry in shared_change event"))?
				.clone(),
		)
		.map_err(|e| anyhow::anyhow!("Failed to parse SharedChangeEntry: {}", e))?;

		debug!(
			hlc = %entry.hlc,
			model_type = %entry.model_type,
			"Broadcasting shared change from event"
		);

		// Broadcast to peers (entry is already in peer_log via TransactionManager)
		let message = SyncMessage::SharedChange {
			library_id,
			entry: entry.clone(),
		};

		let current_state = *state.read().await;
		if current_state.should_buffer() {
			debug!("Buffering own shared change during backfill");
			buffer
				.push(super::state::BufferedUpdate::SharedChange(entry))
				.await;
			return Ok(());
		}

		// Get all connected sync partners
		let connected_partners = network.get_connected_sync_partners().await.map_err(|e| {
			warn!(error = %e, "Failed to get connected partners");
			e
		})?;

		if connected_partners.is_empty() {
			debug!("No connected sync partners to broadcast to");
			return Ok(());
		}

		debug!(
			hlc = %entry.hlc,
			model_type = %entry.model_type,
			partner_count = connected_partners.len(),
			"Broadcasting shared change to sync partners"
		);

		// Broadcast to all partners in parallel
		use futures::future::join_all;

		let send_futures: Vec<_> = connected_partners
			.iter()
			.map(|&partner| {
				let network = network.clone();
				let msg = message.clone();
				async move {
					match tokio::time::timeout(
						std::time::Duration::from_secs(30),
						network.send_sync_message(partner, msg),
					)
					.await
					{
						Ok(Ok(())) => (partner, Ok(())),
						Ok(Err(e)) => (partner, Err(e)),
						Err(_) => (partner, Err(anyhow::anyhow!("Send timeout after 30s"))),
					}
				}
			})
			.collect();

		let results = join_all(send_futures).await;

		// Process results
		let mut success_count = 0;
		let mut error_count = 0;

		for (partner_uuid, result) in results {
			match result {
				Ok(()) => {
					success_count += 1;
					debug!(partner = %partner_uuid, "Shared change sent successfully");
				}
				Err(e) => {
					error_count += 1;
					warn!(
						partner = %partner_uuid,
						error = %e,
						"Failed to send shared change to partner, enqueuing for retry"
					);
					// Enqueue for retry
					retry_queue.enqueue(partner_uuid, message.clone()).await;
				}
			}
		}

		info!(
			hlc = %entry.hlc,
			model_type = %entry.model_type,
			success = success_count,
			errors = error_count,
			"Shared change broadcast complete"
		);

		Ok(())
	}

	/// Stop the sync service
	pub async fn stop(&self) -> Result<()> {
		if !self.is_running.load(Ordering::SeqCst) {
			return Ok(());
		}

		info!(
			library_id = %self.library_id,
			"Stopping peer sync service"
		);

		self.is_running.store(false, Ordering::SeqCst);

		Ok(())
	}

	/// Get current sync state
	pub async fn state(&self) -> DeviceSyncState {
		*self.state.read().await
	}

	/// Broadcast state change (device-owned data)
	pub async fn broadcast_state_change(&self, change: StateChangeMessage) -> Result<()> {
		let state = self.state().await;

		if state.should_buffer() {
			// Still backfilling, buffer our own changes for later broadcast
			debug!("Buffering own state change during backfill");
			self.buffer
				.push(super::state::BufferedUpdate::StateChange(change))
				.await;
			return Ok(());
		}

		// Get all connected sync partners
		let connected_partners = self
			.network
			.get_connected_sync_partners()
			.await
			.map_err(|e| {
				warn!(error = %e, "Failed to get connected partners");
				e
			})?;

		if connected_partners.is_empty() {
			debug!("No connected sync partners to broadcast to");
			return Ok(());
		}

		// Create sync message
		let message = SyncMessage::StateChange {
			library_id: self.library_id,
			model_type: change.model_type.clone(),
			record_uuid: change.record_uuid,
			device_id: change.device_id,
			data: change.data.clone(),
			timestamp: Utc::now(),
		};

		debug!(
			model_type = %change.model_type,
			record_uuid = %change.record_uuid,
			partner_count = connected_partners.len(),
			"Broadcasting state change to sync partners"
		);

		// Broadcast to all partners in parallel using futures::join_all
		use futures::future::join_all;

		let send_futures: Vec<_> = connected_partners
			.iter()
			.map(|&partner| {
				let network = self.network.clone();
				let msg = message.clone();
				async move {
					// Add timeout to prevent hanging indefinitely
					match tokio::time::timeout(
						std::time::Duration::from_secs(30),
						network.send_sync_message(partner, msg),
					)
					.await
					{
						Ok(Ok(())) => (partner, Ok(())),
						Ok(Err(e)) => (partner, Err(e)),
						Err(_) => (partner, Err(anyhow::anyhow!("Send timeout after 30s"))),
					}
				}
			})
			.collect();

		let results = join_all(send_futures).await;

		// Process results
		let mut success_count = 0;
		let mut error_count = 0;

		for (partner_uuid, result) in results {
			match result {
				Ok(()) => {
					success_count += 1;
					debug!(partner = %partner_uuid, "State change sent successfully");
				}
				Err(e) => {
					error_count += 1;
					warn!(
						partner = %partner_uuid,
						error = %e,
						"Failed to send state change to partner, enqueuing for retry"
					);
					// Enqueue for retry
					self.retry_queue
						.enqueue(partner_uuid, message.clone())
						.await;
				}
			}
		}

		info!(
			model_type = %change.model_type,
			success = success_count,
			errors = error_count,
			"State change broadcast complete"
		);

		Ok(())
	}

	/// Broadcast shared change (log-based with HLC)
	pub async fn broadcast_shared_change(
		&self,
		model_type: String,
		record_uuid: Uuid,
		change_type: crate::infra::sync::ChangeType,
		data: serde_json::Value,
	) -> Result<()> {
		// Generate HLC
		let hlc = self.hlc_generator.lock().await.next();

		// Create entry
		let entry = SharedChangeEntry {
			hlc,
			model_type: model_type.clone(),
			record_uuid,
			change_type,
			data,
		};

		// Write to our peer log
		self.peer_log
			.append(entry.clone())
			.await
			.map_err(|e| anyhow::anyhow!("Failed to append to peer log: {}", e))?;

		// Broadcast to peers (if ready)
		let state = self.state().await;
		if state.should_buffer() {
			debug!("Buffering own shared change during backfill");
			self.buffer
				.push(super::state::BufferedUpdate::SharedChange(entry.clone()))
				.await;
			return Ok(());
		}

		// Get all connected sync partners
		let connected_partners = self
			.network
			.get_connected_sync_partners()
			.await
			.map_err(|e| {
				warn!(error = %e, "Failed to get connected partners");
				e
			})?;

		if connected_partners.is_empty() {
			debug!("No connected sync partners to broadcast to");
			return Ok(());
		}

		// Create sync message
		let message = SyncMessage::SharedChange {
			library_id: self.library_id,
			entry: entry.clone(),
		};

		debug!(
			hlc = %hlc,
			model_type = %model_type,
			record_uuid = %record_uuid,
			partner_count = connected_partners.len(),
			"Broadcasting shared change to sync partners"
		);

		// Broadcast to all partners in parallel using futures::join_all
		use futures::future::join_all;

		let send_futures: Vec<_> = connected_partners
			.iter()
			.map(|&partner| {
				let network = self.network.clone();
				let msg = message.clone();
				async move {
					// Add timeout to prevent hanging indefinitely
					match tokio::time::timeout(
						std::time::Duration::from_secs(30),
						network.send_sync_message(partner, msg),
					)
					.await
					{
						Ok(Ok(())) => (partner, Ok(())),
						Ok(Err(e)) => (partner, Err(e)),
						Err(_) => (partner, Err(anyhow::anyhow!("Send timeout after 30s"))),
					}
				}
			})
			.collect();

		let results = join_all(send_futures).await;

		// Process results
		let mut success_count = 0;
		let mut error_count = 0;

		for (partner_uuid, result) in results {
			match result {
				Ok(()) => {
					success_count += 1;
					debug!(partner = %partner_uuid, "Shared change sent successfully");
				}
				Err(e) => {
					error_count += 1;
					warn!(
						partner = %partner_uuid,
						error = %e,
						"Failed to send shared change to partner, enqueuing for retry"
					);
					// Enqueue for retry
					self.retry_queue
						.enqueue(partner_uuid, message.clone())
						.await;
				}
			}
		}

		info!(
			hlc = %hlc,
			model_type = %model_type,
			success = success_count,
			errors = error_count,
			"Shared change broadcast complete"
		);

		Ok(())
	}

	/// Handle received state change
	pub async fn on_state_change_received(&self, change: StateChangeMessage) -> Result<()> {
		let state = self.state().await;

		if state.should_buffer() {
			// Buffer during backfill/catch-up
			self.buffer
				.push(super::state::BufferedUpdate::StateChange(change))
				.await;
			debug!("Buffered state change during backfill");
			return Ok(());
		}

		// Apply immediately
		self.apply_state_change(change).await
	}

	/// Handle received shared change
	pub async fn on_shared_change_received(&self, entry: SharedChangeEntry) -> Result<()> {
		// Update causality
		self.hlc_generator.lock().await.update(entry.hlc);

		let state = self.state().await;

		if state.should_buffer() {
			// Buffer during backfill/catch-up
			let hlc = entry.hlc;
			self.buffer
				.push(super::state::BufferedUpdate::SharedChange(entry))
				.await;
			debug!(
				hlc = %hlc,
				"Buffered shared change during backfill"
			);
			return Ok(());
		}

		// Apply immediately
		self.apply_shared_change(entry).await
	}

	/// Apply state change to database
	async fn apply_state_change(&self, change: StateChangeMessage) -> Result<()> {
		debug!(
			model_type = %change.model_type,
			record_uuid = %change.record_uuid,
			device_id = %change.device_id,
			"Applying state change"
		);

		// Use the registry to route to the appropriate apply function
		crate::infra::sync::apply_state_change(
			&change.model_type,
			change.data.clone(),
			self.db.clone(),
		)
		.await
		.map_err(|e| anyhow::anyhow!("Failed to apply state change: {}", e))?;

		info!(
			model_type = %change.model_type,
			record_uuid = %change.record_uuid,
			"State change applied successfully"
		);

		// Emit event
		self.event_bus.emit(Event::Custom {
			event_type: format!("{}_synced", change.model_type),
			data: serde_json::json!({
				"library_id": self.library_id,
				"record_uuid": change.record_uuid,
				"device_id": change.device_id,
			}),
		});

		Ok(())
	}

	/// Apply shared change to database with conflict resolution
	async fn apply_shared_change(&self, entry: SharedChangeEntry) -> Result<()> {
		debug!(
			hlc = %entry.hlc,
			model_type = %entry.model_type,
			record_uuid = %entry.record_uuid,
			"Applying shared change"
		);

		// Use the registry to route to the appropriate apply function
		// (which handles conflict resolution with HLC)
		crate::infra::sync::apply_shared_change(entry.clone(), self.db.clone())
			.await
			.map_err(|e| anyhow::anyhow!("Failed to apply shared change: {}", e))?;

		info!(
			hlc = %entry.hlc,
			model_type = %entry.model_type,
			record_uuid = %entry.record_uuid,
			"Shared change applied successfully"
		);

		// Send ACK to sender for pruning
		let sender_device_id = entry.hlc.device_id;
		let up_to_hlc = entry.hlc;

		// Don't send ACK to ourselves
		if sender_device_id != self.device_id {
			let ack_message = SyncMessage::AckSharedChanges {
				library_id: self.library_id,
				from_device: self.device_id,
				up_to_hlc,
			};

			debug!(
				sender = %sender_device_id,
				hlc = %up_to_hlc,
				"Sending ACK for shared change"
			);

			// Send ACK (don't fail the whole operation if ACK send fails)
			if let Err(e) = self
				.network
				.send_sync_message(sender_device_id, ack_message)
				.await
			{
				warn!(
					sender = %sender_device_id,
					hlc = %up_to_hlc,
					error = %e,
					"Failed to send ACK to sender (non-fatal)"
				);
			} else {
				debug!(
					sender = %sender_device_id,
					hlc = %up_to_hlc,
					"ACK sent successfully"
				);
			}
		}

		// Emit event
		self.event_bus.emit(Event::Custom {
			event_type: format!("{}_synced", entry.model_type),
			data: serde_json::json!({
				"library_id": self.library_id,
				"record_uuid": entry.record_uuid,
				"hlc": entry.hlc.to_string(),
			}),
		});

		Ok(())
	}

	/// Record ACK from peer and prune
	pub async fn on_ack_received(&self, peer_id: Uuid, up_to_hlc: HLC) -> Result<()> {
		// Record ACK
		self.peer_log
			.record_ack(peer_id, up_to_hlc)
			.await
			.map_err(|e| anyhow::anyhow!("Failed to record ACK: {}", e))?;

		// Try to prune
		let pruned = self
			.peer_log
			.prune_acked()
			.await
			.map_err(|e| anyhow::anyhow!("Failed to prune: {}", e))?;

		if pruned > 0 {
			info!(pruned = pruned, "Pruned shared changes log");
		}

		Ok(())
	}

	/// Get peer log (for testing/TransactionManager integration)
	pub fn peer_log(&self) -> &Arc<crate::infra::sync::PeerLog> {
		&self.peer_log
	}

	/// Get HLC generator (for testing/TransactionManager integration)
	pub fn hlc_generator(&self) -> &Arc<tokio::sync::Mutex<crate::infra::sync::HLCGenerator>> {
		&self.hlc_generator
	}

	/// Get network transport name (for debugging)
	pub fn transport_name(&self) -> &'static str {
		self.network.transport_name()
	}

	/// Get device-owned state for backfill (StateRequest)
	///
	/// This is completely domain-agnostic - it delegates to the Syncable trait
	/// implementations in each entity. No switch statements, no domain logic.
	pub async fn get_device_state(
		&self,
		model_types: Vec<String>,
		device_id: Option<Uuid>,
		since: Option<chrono::DateTime<chrono::Utc>>,
		batch_size: usize,
	) -> Result<Vec<crate::service::network::protocol::sync::messages::StateRecord>> {
		use crate::service::network::protocol::sync::messages::StateRecord;

		debug!(
			model_types = ?model_types,
			device_id = ?device_id,
			since = ?since,
			batch_size = batch_size,
			"Querying device state for backfill"
		);

		let mut all_records = Vec::new();
		let mut remaining_batch = batch_size;

		for model_type in model_types {
			if remaining_batch == 0 {
				break;
			}

			// Query through the registry - completely domain-agnostic
			match crate::infra::sync::registry::query_device_state(
				&model_type,
				device_id,
				since,
				remaining_batch,
				self.db.clone(),
			)
			.await
			{
				Ok(results) => {
					let count = results.len();
					debug!(model_type = %model_type, count = count, "Retrieved records");

					// Convert to StateRecord format
					all_records.extend(results.into_iter().map(|(uuid, data, timestamp)| {
						StateRecord {
							uuid,
							data,
							timestamp,
						}
					}));

					remaining_batch = remaining_batch.saturating_sub(count);
				}
				Err(e) => {
					warn!(
						model_type = %model_type,
						error = %e,
						"Failed to query model type, skipping"
					);
				}
			}
		}

		info!(
			count = all_records.len(),
			"Retrieved device state records for backfill"
		);

		Ok(all_records)
	}

	/// Get shared changes from peer log (SharedChangeRequest)
	pub async fn get_shared_changes(
		&self,
		since_hlc: Option<HLC>,
		limit: usize,
	) -> Result<(Vec<SharedChangeEntry>, bool)> {
		debug!(
			since_hlc = ?since_hlc,
			limit = limit,
			"Querying shared changes from peer log"
		);

		// Query peer log (get all since HLC, then limit in memory)
		let mut entries = self
			.peer_log
			.get_since(since_hlc)
			.await
			.map_err(|e| anyhow::anyhow!("Failed to query peer log: {}", e))?;

		// Check if there are more entries beyond the limit
		let has_more = entries.len() > limit;

		// Truncate to limit
		entries.truncate(limit);

		info!(
			count = entries.len(),
			has_more = has_more,
			"Retrieved shared changes from peer log"
		);

		Ok((entries, has_more))
	}

	/// Transition to ready state (after backfill)
	pub async fn transition_to_ready(&self) -> Result<()> {
		let current_state = self.state().await;

		if !current_state.should_buffer() {
			warn!("Attempted to transition to ready from non-buffering state");
			return Ok(());
		}

		info!("Transitioning to ready, processing buffered updates");

		// Set to catching up
		{
			let mut state = self.state.write().await;
			*state = DeviceSyncState::CatchingUp {
				buffered_count: self.buffer.len().await,
			};
		}

		// Process buffer
		while let Some(update) = self.buffer.pop_ordered().await {
			match update {
				super::state::BufferedUpdate::StateChange(change) => {
					self.apply_state_change(change).await?;
				}
				super::state::BufferedUpdate::SharedChange(entry) => {
					self.apply_shared_change(entry).await?;
				}
			}
		}

		// Now ready!
		{
			let mut state = self.state.write().await;
			*state = DeviceSyncState::Ready;
		}

		info!("Sync service is now ready");

		// Emit event
		self.event_bus.emit(Event::Custom {
			event_type: "sync_ready".to_string(),
			data: serde_json::json!({
				"library_id": self.library_id,
				"device_id": self.device_id,
			}),
		});

		Ok(())
	}
}
