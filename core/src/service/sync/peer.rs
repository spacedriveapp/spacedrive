//! Peer sync service - Leaderless architecture
//!
//! All devices are peers, using hybrid sync:
//! - State-based for device-owned data
//! - Log-based with HLC for shared resources

use crate::{
	infra::{
		event::{Event, EventBus},
		sync::{HLCGenerator, PeerLog, PeerLogError, SharedChangeEntry, HLC},
	},
	library::Library,
};
use anyhow::Result;
use sea_orm::DatabaseConnection;
use std::sync::{
	atomic::{AtomicBool, Ordering},
	Arc,
};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use super::state::{BufferQueue, DeviceSyncState, StateChangeMessage};

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

	/// Whether the service is running
	is_running: Arc<AtomicBool>,
}

impl PeerSync {
	/// Create new peer sync service
	pub async fn new(library: &Library, device_id: Uuid, peer_log: Arc<PeerLog>) -> Result<Self> {
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
			state: Arc::new(RwLock::new(DeviceSyncState::Uninitialized)),
			buffer: Arc::new(BufferQueue::new()),
			hlc_generator: Arc::new(tokio::sync::Mutex::new(HLCGenerator::new(device_id))),
			peer_log,
			event_bus: library.event_bus().clone(),
			is_running: Arc::new(AtomicBool::new(false)),
		})
	}

	/// Get database connection
	pub fn db(&self) -> &Arc<DatabaseConnection> {
		&self.db
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

		// TODO: Start background tasks for:
		// - Listening to network messages
		// - Processing buffer queue
		// - Pruning sync log
		// - Periodic peer health checks

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

		// TODO: Send to all sync_partners via network protocol

		debug!(
			model_type = %change.model_type,
			record_uuid = %change.record_uuid,
			"Broadcast state change"
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
				.push(super::state::BufferedUpdate::SharedChange(entry))
				.await;
			return Ok(());
		}

		// TODO: Send to all sync_partners via network protocol

		debug!(
			hlc = %hlc,
			model_type = %model_type,
			record_uuid = %record_uuid,
			"Broadcast shared change"
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
		// TODO: Deserialize and upsert based on model_type
		debug!(
			model_type = %change.model_type,
			record_uuid = %change.record_uuid,
			device_id = %change.device_id,
			"Applied state change"
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
		// TODO: Deserialize and merge based on model_type
		debug!(
			hlc = %entry.hlc,
			model_type = %entry.model_type,
			record_uuid = %entry.record_uuid,
			"Applied shared change"
		);

		// TODO: Send ACK to sender

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
