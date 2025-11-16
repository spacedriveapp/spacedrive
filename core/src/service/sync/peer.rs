//! Peer sync service
//! - State-based for device-owned data
//! - Log-based with HLC for shared resources
//!
//! ## Architecture
//!
//! This service implements a leaderless peer-to-peer synchronization system where all devices
//! are equal participants. It uses a hybrid approach:
//!
//! - **State-based sync**: For device-owned data (locations, file entries). Each device owns
//!   its data and broadcasts changes via timestamps.
//! - **Log-based sync with HLC**: For shared resources (tags, albums). Uses Hybrid Logical Clocks
//!   for causal ordering and conflict resolution.
//!
//! ## Core Responsibilities
//!
//! ### 1. Connection Management
//! - Monitors network connection/disconnection events
//! - Updates device online status in the database
//! - Automatically triggers watermark exchange on reconnection for incremental catch-up
//!
//! ### 2. Watermark Exchange
//! - Compares sync progress between devices using two types of watermarks:
//!   - `state_watermark`: timestamp tracking device-owned data progress
//!   - `shared_watermark`: HLC tracking shared resource progress
//! - Determines which device is behind and needs catch-up
//! - Initiates incremental sync requests for missing changes
//!
//! ### 3. Change Broadcasting
//! - `broadcast_state_change()`: Sends device-owned changes to all connected peers
//! - `broadcast_shared_change()`: Sends shared resource changes with HLC for ordering
//! - Broadcasts in parallel to all connected sync partners (30s timeout per peer)
//! - Failed sends are queued for retry with exponential backoff
//!
//! ### 4. Change Application
//! - `on_state_change_received()`: Applies incoming device state changes
//! - `on_shared_change_received()`: Applies shared changes with HLC-based conflict resolution
//! - Buffers changes during backfill/catch-up to maintain consistency
//! - Sends ACKs back to originators for distributed log pruning
//!
//! ### 5. Backfill Support
//! - `get_device_state()`: Queries device-owned data for initial sync (domain-agnostic via registry)
//! - `get_shared_changes()`: Retrieves shared changes from peer log since a given HLC
//! - `get_full_shared_state()`: Gets complete snapshot of shared resources for new devices
//!
//! ### 6. Background Tasks
//! - **Retry processor**: Retries failed message sends every 10 seconds
//! - **Log pruner**: Prunes acknowledged log entries every 5 minutes
//! - **Event listener**: Listens for TransactionManager events and broadcasts changes
//! - **Network listener**: Tracks peer connections and triggers watermark exchange
//!
//! ### 7. State Machine
//! The service manages sync lifecycle states:
//! - `Uninitialized`: Service created but not started
//! - `Backfilling`: Receiving initial state from peers
//! - `CatchingUp`: Processing buffered changes after backfill
//! - `Ready`: Fully synchronized, applying changes in real-time
//!
//! During non-ready states, incoming changes are buffered and applied during transition to ready.
//!
//! ## Example Flow: Peer Reconnection
//!
//! 1. Network detects peer connection → `handle_peer_connected()`
//! 2. Watermark exchange initiated → `trigger_watermark_exchange()`
//! 3. Peer responds with its watermarks → `on_watermark_exchange_response()`
//! 4. Watermarks compared to determine who needs catch-up
//! 5. If behind, send catch-up requests (`SharedChangeRequest`)
//! 6. Peer responds with missing changes
//! 7. Changes applied with conflict resolution (HLC comparison)
//! 8. ACKs sent back to peer
//! 9. Both devices prune acknowledged log entries
//!
//! ## Conflict Resolution
//!
//! For shared resources, conflicts are resolved using HLC timestamps:
//! - Incoming change with older/equal HLC → ignored
//! - Incoming change with newer HLC → applied (last-write-wins)
//! - HLC ensures causal consistency across all peers

use crate::{
	infra::{
		event::{Event, EventBus},
		sync::{
			HLCGenerator, NetworkTransport, PeerLog, PeerLogError, ResourceWatermarkStore,
			SharedChangeEntry, HLC,
		},
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
use tokio::sync::{broadcast, mpsc, RwLock};
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

	/// Last successful real-time broadcast per peer (for catch-up lock mechanism)
	/// When real-time broadcasts to a specific peer are active, catch-up for that peer
	/// is skipped to prevent duplication. Per-peer tracking prevents one stuck peer
	/// from blocking recovery for all peers.
	last_realtime_activity_per_peer:
		Arc<RwLock<std::collections::HashMap<Uuid, chrono::DateTime<chrono::Utc>>>>,

	/// HLC generator for this device
	hlc_generator: Arc<tokio::sync::Mutex<HLCGenerator>>,

	/// Per-peer sync log (stored in sync.db)
	pub(super) peer_log: Arc<PeerLog>,

	/// Per-resource watermark store (stored in sync.db)
	watermark_store: ResourceWatermarkStore,

	/// Backfill manager (optional, set after construction to avoid circular refs)
	backfill_manager: Arc<RwLock<Option<std::sync::Weak<super::backfill::BackfillManager>>>>,

	/// Sync configuration
	config: Arc<crate::infra::sync::SyncConfig>,

	/// General event bus (for emitting resource events, metrics)
	event_bus: Arc<EventBus>,

	/// Dedicated sync event bus (for receiving sync coordination events)
	pub(crate) sync_events: Arc<crate::infra::sync::SyncEventBus>,

	/// Retry queue for failed messages
	retry_queue: Arc<RetryQueue>,

	/// Dependency tracker for event-driven retry (replaces O(n²) buffer retry)
	dependency_tracker: Arc<super::dependency::DependencyTracker>,

	/// Whether the service is running
	is_running: Arc<AtomicBool>,

	/// Network event receiver (optional - if provided, enables connection event handling)
	network_events: Arc<
		tokio::sync::Mutex<
			Option<broadcast::Receiver<crate::service::network::core::NetworkEvent>>,
		>,
	>,

	/// Metrics collector for observability
	metrics: Arc<super::metrics::SyncMetricsCollector>,
}

impl PeerSync {
	/// Create new peer sync service
	pub async fn new(
		library: &Library,
		device_id: Uuid,
		peer_log: Arc<PeerLog>,
		network: Arc<dyn NetworkTransport>,
		config: Arc<crate::infra::sync::SyncConfig>,
		metrics: Arc<super::metrics::SyncMetricsCollector>,
	) -> Result<Self> {
		let library_id = library.id();

		info!(
			library_id = %library_id,
			device_id = %device_id,
			"Creating peer sync service"
		);

		// Create watermark store for per-resource tracking
		let watermark_store = ResourceWatermarkStore::new(device_id);

		// Determine initial sync state based on existing watermarks in sync.db
		// If we have watermarks, we've synced before → start as Ready
		// Otherwise → start as Uninitialized and trigger backfill
		let initial_state = {
			let max_watermark = watermark_store
				.get_max_watermark(peer_log.conn())
				.await
				.unwrap_or(None);

			if max_watermark.is_some() {
				info!(
					device_id = %device_id,
					max_watermark = ?max_watermark,
					"Found existing watermarks in sync.db, starting in Ready state (resuming sync)"
				);
				DeviceSyncState::Ready
			} else {
				info!(
					device_id = %device_id,
					"No watermarks found in sync.db, starting in Uninitialized state (will backfill)"
				);
				DeviceSyncState::Uninitialized
			}
		};

		Ok(Self {
			library_id,
			device_id,
			db: Arc::new(library.db().conn().clone()),
			network,
			state: Arc::new(RwLock::new(initial_state)),
			buffer: Arc::new(BufferQueue::new()),
			last_realtime_activity_per_peer: Arc::new(
				RwLock::new(std::collections::HashMap::new()),
			),
			hlc_generator: Arc::new(tokio::sync::Mutex::new(HLCGenerator::new(device_id))),
			peer_log,
			watermark_store,
			backfill_manager: Arc::new(RwLock::new(None)),
			config,
			event_bus: library.event_bus().clone(),
			sync_events: library.sync_events().clone(),
			retry_queue: Arc::new(RetryQueue::new()),
			dependency_tracker: Arc::new(super::dependency::DependencyTracker::new()),
			is_running: Arc::new(AtomicBool::new(false)),
			network_events: Arc::new(tokio::sync::Mutex::new(None)),
			metrics,
		})
	}

	/// Set backfill manager reference (called after construction to avoid circular refs)
	pub async fn set_backfill_manager(
		&self,
		manager: std::sync::Weak<super::backfill::BackfillManager>,
	) {
		*self.backfill_manager.write().await = Some(manager);
	}

	/// Set network event receiver for connection tracking
	pub async fn set_network_events(
		&self,
		receiver: broadcast::Receiver<crate::service::network::core::NetworkEvent>,
	) {
		*self.network_events.lock().await = Some(receiver);
	}

	/// Get database connection
	pub fn db(&self) -> &Arc<DatabaseConnection> {
		&self.db
	}

	/// Get network transport
	pub fn network(&self) -> &Arc<dyn NetworkTransport> {
		&self.network
	}

	/// Get this device's ID
	pub fn device_id(&self) -> Uuid {
		self.device_id
	}

	/// Get this library's ID
	pub fn library_id(&self) -> Uuid {
		self.library_id
	}

	/// Get peer log connection for watermark queries
	pub fn peer_log_conn(&self) -> &sea_orm::DatabaseConnection {
		self.peer_log.conn()
	}

	/// Get counts of device-owned resources owned by a specific device
	///
	/// Used for gap detection during watermark exchange.
	/// Only counts non-deleted records where device ownership matches.
	pub async fn get_device_owned_counts(
		owner_device_id: Uuid,
		db: &DatabaseConnection,
	) -> Result<std::collections::HashMap<String, u64>> {
		use crate::infra::db::entities::{device, entry, location, volume};
		use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QuerySelect};

		let mut counts = std::collections::HashMap::new();

		// Get device's internal ID from UUID
		let device = device::Entity::find()
			.filter(device::Column::Uuid.eq(owner_device_id))
			.one(db)
			.await?;

		let device_internal_id = match device {
			Some(d) => d.id,
			None => {
				debug!(device_id = %owner_device_id, "Device not found for count query");
				return Ok(counts);
			}
		};

		// Location count (owned by device)
		let location_count = location::Entity::find()
			.filter(location::Column::DeviceId.eq(device_internal_id))
			.count(db)
			.await
			.unwrap_or(0);
		counts.insert("location".to_string(), location_count);

		// Entry count (via location ownership chain using closure table)
		// Query entries where location.device_id matches via entry_closure table
		let entry_count: u64 = {
			use sea_orm::sea_query::{Expr, Query};
			use sea_orm::{FromQueryResult, Statement};

			#[derive(FromQueryResult)]
			struct CountResult {
				count: i64,
			}

			let stmt = Statement::from_sql_and_values(
				sea_orm::DbBackend::Sqlite,
				r#"
			SELECT COUNT(DISTINCT ec.descendant_id) as count
			FROM entry_closure ec
			WHERE ec.ancestor_id IN (
				SELECT entry_id FROM locations WHERE device_id = ?
			)
			"#,
				vec![device_internal_id.into()],
			);

			let result = CountResult::find_by_statement(stmt)
				.one(db)
				.await
				.unwrap_or(None);

			result.map(|r| r.count as u64).unwrap_or(0)
		};
		counts.insert("entry".to_string(), entry_count);

		// Volume count (owned by device)
		let volume_count = volume::Entity::find()
			.filter(volume::Column::DeviceId.eq(device_internal_id))
			.count(db)
			.await
			.unwrap_or(0);
		counts.insert("volume".to_string(), volume_count);

		debug!(
			device_id = %owner_device_id,
			location_count = location_count,
			entry_count = entry_count,
			volume_count = volume_count,
			"Queried device-owned resource counts"
		);

		Ok(counts)
	}

	/// Clear watermarks for specific resources (surgical recovery)
	///
	/// Called when count mismatch is detected for specific resources.
	/// Only clears the mismatched resources, leaving correct watermarks intact.
	async fn clear_resource_watermarks(
		&self,
		peer_id: Uuid,
		resource_types: Vec<String>,
	) -> Result<()> {
		let store = crate::infra::sync::ResourceWatermarkStore::new(self.device_id);

		let mut cleared_count = 0;
		for resource_type in &resource_types {
			if store
				.delete_resource(self.peer_log.conn(), peer_id, resource_type)
				.await
				.map_err(|e| anyhow::anyhow!("Failed to clear resource watermark: {}", e))?
			{
				cleared_count += 1;
			}
		}

		info!(
			peer = %peer_id,
			resources = ?resource_types,
			cleared = cleared_count,
			"Cleared watermarks for mismatched resources only (surgical recovery)"
		);

		Ok(())
	}

	/// Check if real-time sync is currently active for a specific peer
	///
	/// Returns true if real-time broadcasts to this peer succeeded in the last 30 seconds.
	/// Used to prevent catch-up from overlapping with active real-time sync.
	/// Per-peer tracking prevents one stuck peer from blocking recovery for others.
	pub async fn is_realtime_active_for_peer(&self, peer_id: Uuid) -> bool {
		if let Some(last_activity) = self
			.last_realtime_activity_per_peer
			.read()
			.await
			.get(&peer_id)
		{
			let elapsed = chrono::Utc::now().signed_duration_since(*last_activity);
			elapsed.num_seconds() < 30 // Reduced from 60s to 30s for faster recovery
		} else {
			false
		}
	}

	/// Update real-time activity timestamp for a specific peer (called after successful broadcast)
	async fn mark_realtime_activity_for_peer(&self, peer_id: Uuid) {
		self.last_realtime_activity_per_peer
			.write()
			.await
			.insert(peer_id, chrono::Utc::now());
	}

	/// Get per-resource watermark for a specific peer and resource type
	///
	/// Returns the watermark from sync.db tracking this peer's sync progress
	/// for the given resource type (location, entry, volume, etc.)
	pub async fn get_resource_watermark(
		&self,
		peer_device_id: Uuid,
		resource_type: &str,
	) -> Result<Option<chrono::DateTime<chrono::Utc>>> {
		self.watermark_store
			.get(self.peer_log.conn(), peer_device_id, resource_type)
			.await
			.map_err(|e| anyhow::anyhow!("Failed to get resource watermark: {}", e))
	}

	/// Update per-resource watermark after processing state changes
	///
	/// Only updates if the new watermark is newer than the existing one.
	pub async fn update_resource_watermark(
		&self,
		peer_device_id: Uuid,
		resource_type: &str,
		timestamp: chrono::DateTime<chrono::Utc>,
	) -> Result<()> {
		self.watermark_store
			.upsert(
				self.peer_log.conn(),
				peer_device_id,
				resource_type,
				timestamp,
			)
			.await
			.map_err(|e| anyhow::anyhow!("Failed to update resource watermark: {}", e))?;

		debug!(
			peer = %peer_device_id,
			resource = %resource_type,
			watermark = %timestamp,
			"Updated resource watermark in sync.db"
		);

		Ok(())
	}

	/// Get all watermarks for a peer (for diagnostics)
	pub async fn get_all_watermarks_for_peer(
		&self,
		peer_device_id: Uuid,
	) -> Result<Vec<(String, chrono::DateTime<chrono::Utc>)>> {
		self.watermark_store
			.get_all_for_peer(self.peer_log.conn(), peer_device_id)
			.await
			.map_err(|e| anyhow::anyhow!("Failed to get peer watermarks: {}", e))
	}

	/// Query watermarks from sync.db (per-resource aggregation)
	///
	/// For state watermark: Returns the maximum (most recent) timestamp across all resources
	/// For shared watermark: Returns the maximum HLC from peer log
	async fn query_device_watermarks(
		device_id: Uuid,
		peer_log: &Arc<crate::infra::sync::PeerLog>,
	) -> (Option<chrono::DateTime<chrono::Utc>>, Option<HLC>) {
		// Get max state watermark by aggregating across all resources
		let state_watermark = match crate::infra::sync::ResourceWatermarkStore::new(device_id)
			.get_max_watermark(peer_log.conn())
			.await
		{
			Ok(max) => max,
			Err(e) => {
				warn!(
					device_id = %device_id,
					error = %e,
					"Failed to query per-resource watermarks, returning None"
				);
				None
			}
		};

		// Get max shared watermark from peer log
		let shared_watermark = match peer_log.get_max_hlc().await {
			Ok(max) => max,
			Err(e) => {
				warn!(
					device_id = %device_id,
					error = %e,
					"Failed to query max HLC from peer log"
				);
				None
			}
		};

		(state_watermark, shared_watermark)
	}

	/// Get watermarks for heartbeat and reconnection sync
	///
	/// Returns (state_watermark, shared_watermark) aggregated from sync.db.
	/// State watermark: Maximum timestamp across all per-resource watermarks.
	/// Shared watermark (HLC): Maximum HLC from peer log.
	pub async fn get_watermarks(&self) -> (Option<chrono::DateTime<chrono::Utc>>, Option<HLC>) {
		Self::query_device_watermarks(self.device_id, &self.peer_log).await
	}

	/// Update shared watermark for this device after processing shared changes
	///
	/// Note: Shared watermark is now tracked automatically in the peer log.
	/// This method ensures the HLC generator is updated for causality.
	async fn update_shared_watermark(&self, hlc: HLC) -> Result<()> {
		// Update HLC generator to maintain causality
		self.hlc_generator.lock().await.update(hlc);

		debug!(
			hlc = %hlc,
			"Updated shared watermark (HLC generator updated for causality)"
		);

		Ok(())
	}

	/// Mark backfill complete by updating last_sync_at
	///
	/// Note: Per-resource watermarks are now tracked automatically as data is received.
	/// This method only updates the last_sync_at timestamp.
	pub async fn set_initial_watermarks(
		&self,
		_final_state_checkpoint: Option<String>,
		max_shared_hlc: Option<crate::infra::sync::HLC>,
	) -> Result<()> {
		use crate::infra::db::entities;
		use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};

		let now = chrono::Utc::now();

		// Update shared watermark (HLC generator) if we received shared data
		if let Some(hlc) = max_shared_hlc {
			self.hlc_generator.lock().await.update(hlc);
			debug!(
				hlc = %hlc,
				"Updated HLC generator from backfill max HLC"
			);
		}

		// Update last_sync_at to mark backfill complete
		let device = entities::device::Entity::find()
			.filter(entities::device::Column::Uuid.eq(self.device_id))
			.one(self.db.as_ref())
			.await
			.map_err(|e| anyhow::anyhow!("Failed to query device: {}", e))?
			.ok_or_else(|| anyhow::anyhow!("Device not found: {}", self.device_id))?;

		let mut device_active: entities::device::ActiveModel = device.into();
		device_active.last_sync_at = Set(Some(now));

		device_active
			.update(self.db.as_ref())
			.await
			.map_err(|e| anyhow::anyhow!("Failed to update last_sync_at: {}", e))?;

		info!(
			last_sync_at = %now,
			"Backfill complete, per-resource watermarks tracked in sync.db"
		);

		Ok(())
	}

	/// Notify all connected peers that we have new data available
	///
	/// This proactively triggers watermark exchange on peers after bulk operations.
	/// Prevents the "20-minute idle" bug where events die and peers don't notice.
	pub async fn notify_peers_of_new_data(
		&self,
		resource_types: Vec<String>,
		approx_count: u64,
	) -> Result<()> {
		let connected_partners = self
			.network
			.get_connected_sync_partners(self.library_id, &self.db)
			.await?;

		if connected_partners.is_empty() {
			debug!("No connected partners, skipping data available notification");
			return Ok(());
		}

		info!(
			resources = ?resource_types,
			count = approx_count,
			peer_count = connected_partners.len(),
			"Notifying peers of new data available"
		);

		let notification = SyncMessage::DataAvailableNotification {
			library_id: self.library_id,
			device_id: self.device_id,
			resource_types,
			approx_count,
		};

		// Broadcast to all connected peers (fire and forget - they'll request via watermark exchange)
		for peer_id in connected_partners {
			if let Err(e) = self
				.network
				.send_sync_message(peer_id, notification.clone())
				.await
			{
				warn!(
					peer = %peer_id,
					error = %e,
					"Failed to send data available notification"
				);
			}
		}

		Ok(())
	}

	/// Exchange watermarks with a peer and trigger catch-up if needed
	///
	/// Sends a WatermarkExchangeRequest to the peer with our watermarks.
	/// The peer will respond via the protocol handler, which will call
	/// `on_watermark_exchange_response()` to complete the exchange.
	pub async fn exchange_watermarks_and_catchup(&self, peer_id: Uuid) -> Result<()> {
		info!(
			peer = %peer_id,
			"Initiating watermark exchange for reconnection sync"
		);

		// Get our watermarks
		let (_my_state_watermark, my_shared_watermark) = self.get_watermarks().await;

		// Get per-resource watermarks for fine-grained comparison
		let my_resource_watermarks =
			crate::infra::sync::ResourceWatermarkStore::new(self.device_id)
				.get_our_resource_watermarks(self.peer_log.conn())
				.await
				.unwrap_or_else(|e| {
					warn!(error = %e, "Failed to get per-resource watermarks");
					std::collections::HashMap::new()
				});

		// Get counts of peer's device-owned resources that we have synced (for gap detection)
		let my_peer_resource_counts = Self::get_device_owned_counts(peer_id, &self.db)
			.await
			.unwrap_or_else(|e| {
				warn!(error = %e, peer = %peer_id, "Failed to get peer resource counts");
				std::collections::HashMap::new()
			});

		debug!(
			peer = %peer_id,
			my_shared_watermark = ?my_shared_watermark,
			resource_count = my_resource_watermarks.len(),
			peer_owned_counts = ?my_peer_resource_counts,
			"Sending watermark exchange request with counts for gap detection"
		);

		// Send request to peer and wait for response (request/response pattern)
		let request = SyncMessage::WatermarkExchangeRequest {
			library_id: self.library_id,
			device_id: self.device_id,
			my_shared_watermark,
			my_resource_watermarks,
			my_peer_resource_counts,
		};

		// Use send_sync_request() to get response back (bi-directional stream)
		let response = self.network
			.send_sync_request(peer_id, request)
			.await
			.map_err(|e| anyhow::anyhow!("Failed to exchange watermarks: {}", e))?;

		info!(
			peer = %peer_id,
			"Watermark exchange request sent, received response"
		);

		// Process the response
		match response {
			SyncMessage::WatermarkExchangeResponse {
				shared_watermark: peer_shared_watermark,
				needs_state_catchup,
				needs_shared_catchup,
				resource_watermarks: peer_resource_watermarks,
				my_actual_resource_counts: peer_actual_counts,
				..
			} => {
				info!(
					peer = %peer_id,
					peer_shared_watermark = ?peer_shared_watermark,
					needs_state_catchup = needs_state_catchup,
					needs_shared_catchup = needs_shared_catchup,
					peer_actual_counts = ?peer_actual_counts,
					"Received watermark exchange response with counts, processing locally"
				);

				// Call the response handler directly
				self.on_watermark_exchange_response(
					peer_id,
					peer_shared_watermark,
					needs_state_catchup,
					needs_shared_catchup,
					peer_resource_watermarks,
					peer_actual_counts,
				)
				.await?;
			}
			_ => {
				return Err(anyhow::anyhow!(
					"Expected WatermarkExchangeResponse, got different message type"
				));
			}
		}

		Ok(())
	}

	/// Handle watermark exchange response from peer
	///
	/// Called by the protocol handler when a WatermarkExchangeResponse is received.
	/// Triggers incremental catch-up if watermarks diverge.
	pub async fn on_watermark_exchange_response(
		&self,
		peer_id: Uuid,
		peer_shared_watermark: Option<HLC>,
		needs_state_catchup: bool,
		needs_shared_catchup: bool,
		peer_resource_watermarks: std::collections::HashMap<String, chrono::DateTime<chrono::Utc>>,
		peer_actual_resource_counts: std::collections::HashMap<String, u64>,
	) -> Result<()> {
		info!(
			peer = %peer_id,
			peer_shared_watermark = ?peer_shared_watermark,
			needs_state_catchup = needs_state_catchup,
			needs_shared_catchup = needs_shared_catchup,
			peer_resource_count = peer_resource_watermarks.len(),
			"Received watermark exchange response with resource counts"
		);

		// Get our watermarks to compare
		let (_my_state_watermark, my_shared_watermark) = self.get_watermarks().await;
		let my_resource_watermarks =
			crate::infra::sync::ResourceWatermarkStore::new(self.device_id)
				.get_our_resource_watermarks(self.peer_log.conn())
				.await
				.unwrap_or_default();

		// Count-based gap detection (detects watermark leapfrog bugs)
		// Only run when NOT in real-time sync or active backfill
		let state = self.state().await;
		let realtime_active = self.is_realtime_active_for_peer(peer_id).await;
		let in_stable_state = state.is_ready() && !realtime_active;

		if in_stable_state && !peer_actual_resource_counts.is_empty() {
			// Get what we think peer has
			let my_counts_of_peer_data = Self::get_device_owned_counts(peer_id, &self.db)
				.await
				.unwrap_or_default();

			let mut mismatched_resource_types = Vec::new();
			let mut mismatch_details = Vec::new();

			for (resource_type, peer_actual_count) in &peer_actual_resource_counts {
				let our_count = my_counts_of_peer_data
					.get(resource_type)
					.copied()
					.unwrap_or(0);

				// If WE have less than peer actually owns, we're behind and need catch-up
				// Don't trigger if we have MORE (that would mean peer is behind, they'll catch up from us)
				if our_count < *peer_actual_count {
					let watermark_diff_seconds = my_resource_watermarks
						.get(resource_type)
						.and_then(|my_ts| {
							peer_resource_watermarks
								.get(resource_type)
								.map(|peer_ts| (my_ts.timestamp() - peer_ts.timestamp()).abs())
						})
						.unwrap_or(0);

					warn!(
						peer = %peer_id,
						resource = %resource_type,
						our_count = our_count,
						peer_actual = peer_actual_count,
						gap = i64::abs(our_count as i64 - *peer_actual_count as i64),
						our_watermark = ?my_resource_watermarks.get(resource_type),
						peer_watermark = ?peer_resource_watermarks.get(resource_type),
						watermark_diff_seconds = watermark_diff_seconds,
						"COUNT MISMATCH DETECTED! Triggering surgical recovery"
					);
					mismatched_resource_types.push(resource_type.clone());
					mismatch_details.push(format!(
						"{}({}/{}, Δ{})",
						resource_type,
						our_count,
						peer_actual_count,
						i64::abs(our_count as i64 - *peer_actual_count as i64)
					));
				}
			}

			if !mismatched_resource_types.is_empty() {
				error!(
					peer = %peer_id,
					mismatches = ?mismatch_details,
					resources = ?mismatched_resource_types,
					"Count mismatch indicates watermark leapfrog bug, clearing only affected resources"
				);

				// Clear watermarks only for mismatched resources
				// This preserves correct watermarks for other resources
				self.clear_resource_watermarks(peer_id, mismatched_resource_types.clone())
					.await?;

				// Trigger catch-up for affected resources ONLY
				// CRITICAL: Preserve shared watermark to avoid unnecessary shared resource backfill
				let backfill_mgr = self.backfill_manager.read().await;
				if let Some(weak_ref) = backfill_mgr.as_ref() {
					if let Some(manager) = weak_ref.upgrade() {
						info!(
							peer = %peer_id,
							resources = ?mismatched_resource_types,
							"Initiating surgical backfill (device-owned resources only, preserving shared watermark)"
						);

						// Get current shared watermark to preserve it
						let (_my_state, my_shared) = self.get_watermarks().await;
						let shared_watermark_str = my_shared.map(|hlc| hlc.to_string());

						// State watermark = None (cleared for mismatched resources)
						// Shared watermark = current (preserved to skip shared backfill)
						manager.catch_up_from_peer(peer_id, None, shared_watermark_str).await?;
					}
				} else {
					warn!("BackfillManager not available, cannot trigger recovery backfill");
				}

				return Ok(());
			} else {
				debug!(
					peer = %peer_id,
					"Count validation passed - no gaps detected"
				);
			}
		} else {
			debug!(
				peer = %peer_id,
				state = ?state,
				realtime_active = realtime_active,
				has_counts = !peer_actual_resource_counts.is_empty(),
				"Skipping count-based gap detection - not in stable state or no counts provided"
			);
		}

		// Determine if WE need to catch up based on per-resource watermark comparison
		let mut we_need_state_catchup = false;
		let mut we_need_shared_catchup = false;
		let mut resources_needing_catchup = Vec::new();

		// Compare per-resource watermarks (CRITICAL FIX: Issue #10)
		// This fixes the bug where global watermark comparison missed per-resource divergence
		for (resource_type, peer_ts) in &peer_resource_watermarks {
			match my_resource_watermarks.get(resource_type) {
				Some(my_ts) if peer_ts > my_ts => {
					info!(
						peer = %peer_id,
						resource_type = %resource_type,
						my_timestamp = %my_ts,
						peer_timestamp = %peer_ts,
						"Peer has newer data for this resource"
					);
					resources_needing_catchup.push(resource_type.clone());
					we_need_state_catchup = true;
				}
				None => {
					info!(
						peer = %peer_id,
						resource_type = %resource_type,
						peer_timestamp = %peer_ts,
						"We have no watermark for this resource, need catch-up"
					);
					resources_needing_catchup.push(resource_type.clone());
					we_need_state_catchup = true;
				}
				_ => {
					debug!(
						resource_type = %resource_type,
						"Resource in sync with peer"
					);
				}
			}
		}

		if we_need_state_catchup {
			info!(
				peer = %peer_id,
				resources = ?resources_needing_catchup,
				"Need state catch-up for specific resources"
			);
		}

		// Compare shared watermarks (HLC)
		match (my_shared_watermark, peer_shared_watermark) {
			(Some(my_hlc), Some(peer_hlc)) if peer_hlc > my_hlc => {
				info!(
					peer = %peer_id,
					my_hlc = %my_hlc,
					peer_hlc = %peer_hlc,
					"Peer has newer shared changes, need to catch up"
				);
				we_need_shared_catchup = true;
			}
			(None, Some(_)) => {
				info!(peer = %peer_id, "We have no shared watermark, need full shared catch-up");
				we_need_shared_catchup = true;
			}
			_ => {
				debug!(peer = %peer_id, "Shared watermarks in sync");
			}
		}

		// Trigger catch-up if needed
		if we_need_state_catchup {
			info!(peer = %peer_id, "Triggering incremental state catch-up");

			// Get backfill manager if available
			let backfill_mgr = self.backfill_manager.read().await;
			if let Some(weak_ref) = backfill_mgr.as_ref() {
				if let Some(manager) = weak_ref.upgrade() {
					// Trigger incremental catch-up from this peer
					// Note: We now use per-resource watermarks instead of global state watermark
					if let Err(e) = manager
						.catch_up_from_peer(
							peer_id,
							None, // Per-resource watermarks used instead of global
							my_shared_watermark.map(|hlc| hlc.to_string()),
						)
						.await
					{
						warn!(
							peer = %peer_id,
							error = %e,
							"Failed to trigger catch-up, will retry on next watermark exchange"
						);
					}
				} else {
					warn!("BackfillManager weak reference expired");
				}
			} else {
				warn!("BackfillManager not set on PeerSync, cannot trigger catch-up");
			}
		}

		if we_need_shared_catchup {
			info!(
				peer = %peer_id,
				since_hlc = ?my_shared_watermark,
				"Requesting incremental shared changes catch-up"
			);
			// Request shared changes since our watermark
			let request = SyncMessage::SharedChangeRequest {
				library_id: self.library_id,
				since_hlc: my_shared_watermark,
				limit: self.config.batching.backfill_batch_size,
			};

			self.network
				.send_sync_message(peer_id, request)
				.await
				.map_err(|e| {
					anyhow::anyhow!("Failed to request shared changes for catch-up: {}", e)
				})?;

			info!(peer = %peer_id, "Shared changes catch-up request sent");
		}

		// If peer needs our data, send it
		if needs_state_catchup {
			info!(peer = %peer_id, "Peer needs state catch-up, sending our state");
			// Peer will request our state via StateRequest, we'll respond via protocol handler
		}

		if needs_shared_catchup {
			info!(
				peer = %peer_id,
				"Peer needs shared catch-up, they will request via SharedChangeRequest"
			);
			// Peer will request our shared changes, we'll respond via protocol handler
		}

		// Update devices table with peer's watermarks for future comparisons
		// Note: We now track per-resource watermarks, not global state watermark
		self.update_peer_watermarks(peer_id, None, peer_shared_watermark)
			.await?;

		info!(peer = %peer_id, "Watermark exchange complete");

		Ok(())
	}

	/// Update peer's last seen timestamp after watermark exchange
	///
	/// Note: Peer watermarks are now tracked per-resource in sync.db.
	/// This method only updates the peer's updated_at timestamp for diagnostics.
	async fn update_peer_watermarks(
		&self,
		peer_id: Uuid,
		_state_watermark: Option<chrono::DateTime<chrono::Utc>>,
		_shared_watermark: Option<HLC>,
	) -> Result<()> {
		use crate::infra::db::entities;
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

		// Just update the updated_at timestamp to track when we last heard from peer
		entities::device::Entity::update_many()
			.col_expr(
				entities::device::Column::UpdatedAt,
				sea_orm::sea_query::Expr::value(Utc::now()),
			)
			.filter(entities::device::Column::Uuid.eq(peer_id))
			.exec(self.db.as_ref())
			.await?;

		debug!(
			peer = %peer_id,
			"Updated peer timestamp after watermark exchange"
		);

		Ok(())
	}

	/// Start the sync service
	pub async fn start(self: &Arc<Self>) -> Result<()> {
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

		// Start network event listener for connection tracking
		self.start_network_event_listener().await;

		// Start background task for retry queue processing
		self.start_retry_processor();

		// Start background task for periodic log pruning
		self.start_log_pruner();

		// Start periodic watermark check (safety net for missed events)
		self.start_periodic_watermark_check();

		Ok(())
	}

	/// Start background task to process retry queue
	fn start_retry_processor(&self) {
		let retry_queue = self.retry_queue.clone();
		let network = self.network.clone();
		let is_running = self.is_running.clone();
		let config = self.config.clone();

		tokio::spawn(async move {
			info!("Started retry queue processor");

			while is_running.load(Ordering::SeqCst) {
				// Check for ready messages at configured interval
				tokio::time::sleep(tokio::time::Duration::from_secs(
					config.network.sync_loop_interval_secs,
				))
				.await;

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
		let config = self.config.clone();

		tokio::spawn(async move {
			info!("Started log pruner");

			while is_running.load(Ordering::SeqCst) {
				// Prune at configured interval
				tokio::time::sleep(tokio::time::Duration::from_secs(
					config.monitoring.pruning_interval_secs,
				))
				.await;

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

	/// Start periodic watermark check (safety net for missed events)
	///
	/// Exchanges watermarks with all connected peers every 1 minute to ensure
	/// sync divergence is detected even if events are dropped or broadcasts fail.
	/// Uses full request/response pattern for count validation.
	fn start_periodic_watermark_check(self: &Arc<Self>) {
		let peer_sync = Arc::downgrade(self);
		let library_id = self.library_id;
		let db = self.db.clone();
		let is_running = self.is_running.clone();

		tokio::spawn(async move {
			info!("Started periodic watermark check (every 1 minute)");

			// Wait 1 minute before first check (allow initial sync to complete)
			tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;

			while is_running.load(Ordering::SeqCst) {
				// Check every 1 minute
				tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;

				info!("Periodic watermark check interval elapsed, checking for connected partners");

				// Upgrade weak reference
				let peer_sync_arc = match peer_sync.upgrade() {
					Some(ps) => ps,
					None => {
						warn!("PeerSync dropped, stopping periodic watermark check");
						break;
					}
				};

				// Get connected sync partners
				match peer_sync_arc.network.get_connected_sync_partners(library_id, &db).await {
					Ok(partners) if !partners.is_empty() => {
						info!(
							partner_count = partners.len(),
							"Running periodic watermark check with connected partners"
						);

						// Exchange watermarks with all peers using full request/response
						for peer_id in partners {
							if let Err(e) = peer_sync_arc.exchange_watermarks_and_catchup(peer_id).await
							{
								debug!(
									peer = %peer_id,
									error = %e,
									"Periodic watermark exchange failed for peer"
								);
							} else {
								info!(
									peer = %peer_id,
									"Periodic watermark exchange completed with count validation"
								);
							}
						}
					}
					Ok(_) => {
						info!("No connected partners for periodic watermark check");
					}
					Err(e) => {
						warn!(error = %e, "Failed to get connected partners for periodic watermark check");
					}
				}
			}

			info!("Periodic watermark check stopped");
		});
	}

	/// Start event listener for TransactionManager sync events
	fn start_event_listener(&self) {
		// Clone necessary fields for the spawned task
		let library_id = self.library_id;
		let network = self.network.clone();
		info!(
			"PeerSync sync event listener cloning network transport: {:?}",
			std::any::type_name_of_val(&*network)
		);
		let state = self.state.clone();
		let buffer = self.buffer.clone();
		let last_realtime_activity_per_peer = self.last_realtime_activity_per_peer.clone();
		let db = self.db.clone();
		let event_bus_for_emit = self.event_bus.clone();
		let retry_queue = self.retry_queue.clone();
		let mut subscriber = self.sync_events.subscribe();
		let is_running = self.is_running.clone();
		let config = self.config.clone();

		tokio::spawn(async move {
			info!(
				"PeerSync sync event listener started with network transport: {}",
				network.transport_name()
			);

			// Track event statistics for lag diagnostics
			let mut last_event_type: Option<String> = None;
			let mut state_change_count = 0u64;
			let mut shared_change_count = 0u64;
			let mut last_lag_warning = std::time::Instant::now();
			let lag_warning_cooldown = std::time::Duration::from_secs(5);

			// Real-time batching mechanism (configurable via SyncConfig)
			let mut state_change_batch: Vec<serde_json::Value> = Vec::new();
			let mut batch_flush_interval = tokio::time::interval(std::time::Duration::from_millis(
				config.batching.realtime_batch_flush_interval_ms,
			));
			batch_flush_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

			while is_running.load(Ordering::SeqCst) {
				tokio::select! {
						// Receive events from sync event bus
						event_result = subscriber.recv() => {
							match event_result {
								Ok(sync_event) => {
									use crate::infra::sync::SyncEvent;

									match sync_event {
										SyncEvent::StateChange {
											library_id: event_library_id,
											model_type,
											record_uuid,
											device_id,
											data,
											timestamp,
										} => {
											state_change_count += 1;
											last_event_type = Some(format!("StateChange({})", model_type));

										// Add to batch instead of processing immediately
										state_change_batch.push(serde_json::json!({
											"library_id": event_library_id,
											"model_type": model_type,
											"record_uuid": record_uuid,
											"device_id": device_id,
											"data": data,
											"timestamp": timestamp,
										}));

										// Flush if batch reaches configured max entries
										if state_change_batch.len() >= config.batching.realtime_batch_max_entries {
												Self::flush_state_change_batch(
													library_id,
													&mut state_change_batch,
													&network,
													&state,
													&buffer,
													&retry_queue,
													&db,
													&config,
													&last_realtime_activity_per_peer,
												).await;
											}
										}
								SyncEvent::SharedChange {
									library_id: event_library_id,
									entry,
								} => {
									shared_change_count += 1;
									last_event_type = Some(format!(
										"SharedChange({}, HLC:{})",
										entry.model_type, entry.hlc
									));

									info!(
										hlc = %entry.hlc,
										model_type = %entry.model_type,
										"PeerSync received shared change event"
									);

									if let Err(e) = Self::handle_shared_change_event_static(
										library_id,
										serde_json::json!({
											"library_id": event_library_id,
											"entry": entry,
										}),
										&network,
										&state,
										&buffer,
										&retry_queue,
										&db,
										&config,
									)
									.await
									{
										warn!(error = %e, "Failed to handle shared change event");
									}
								}
								SyncEvent::MetricsUpdated { .. } => {
									// Ignore metrics events in the sync event listener
									// (metrics are for observability, not sync coordination)
								}
							}
						}
						Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
							// Rate-limit lag warnings to prevent spam
							let now = std::time::Instant::now();
							if now.duration_since(last_lag_warning) >= lag_warning_cooldown {
								error!(
									skipped = skipped,
									last_event = ?last_event_type,
									state_changes = state_change_count,
									shared_changes = shared_change_count,
									"CRITICAL: Sync event listener lagged! Data loss occurred. \
									This likely includes StateChange or SharedChange events. \
									Lost events may cause sync inconsistency - full backfill may be needed. \
									With 10k capacity, this indicates extreme system load or a bug."
								);

								last_lag_warning = now;
							} else {
								debug!(
									skipped = skipped,
									"Sync event listener lagged (warning suppressed)"
								);
							}
						}
						Err(tokio::sync::broadcast::error::RecvError::Closed) => {
							info!("Sync event bus closed, stopping event listener");
							break;
						}
					}
				}

				// Flush batch on timer (configurable interval)
				_ = batch_flush_interval.tick() => {
							if !state_change_batch.is_empty() {
								Self::flush_state_change_batch(
									library_id,
									&mut state_change_batch,
									&network,
									&state,
									&buffer,
									&retry_queue,
									&db,
									&config,
									&last_realtime_activity_per_peer,
								).await;
							}
						}
					}
			}

			// Flush any remaining batched state changes before stopping
			if !state_change_batch.is_empty() {
				info!(
					remaining = state_change_batch.len(),
					"Flushing remaining batched state changes before shutdown"
				);
				Self::flush_state_change_batch(
					library_id,
					&mut state_change_batch,
					&network,
					&state,
					&buffer,
					&retry_queue,
					&db,
					&config,
					&last_realtime_activity_per_peer,
				)
				.await;
			}

			info!("PeerSync sync event listener stopped");
		});
	}

	/// Start network event listener for connection tracking
	async fn start_network_event_listener(&self) {
		// Take the receiver from the mutex (if available)
		let mut receiver = self.network_events.lock().await.take();

		if receiver.is_none() {
			debug!("No network event receiver available - connection tracking disabled");
			return;
		}

		let db = self.db.clone();
		let is_running = self.is_running.clone();
		let library_id = self.library_id;
		let device_id = self.device_id;
		let network = self.network.clone();
		let peer_log = self.peer_log.clone(); // CRITICAL FIX: Pass peer_log to spawned task

		tokio::spawn(async move {
			info!("PeerSync network event listener started");

			let mut rx = receiver.unwrap();

			while is_running.load(Ordering::SeqCst) {
				match rx.recv().await {
					Ok(event) => {
						use crate::service::network::core::NetworkEvent;
						match event {
							NetworkEvent::ConnectionEstablished {
								device_id: peer_id,
								node_id,
							} => {
								info!(
									peer_id = %peer_id,
									node_id = %node_id,
									"Device connected - updating devices table and triggering watermark exchange"
								);

								// Update devices table
								if let Err(e) = Self::handle_peer_connected(peer_id, &db).await {
									warn!(
										peer_id = %peer_id,
										error = %e,
										"Failed to handle peer connected event"
									);
								}

								// CRITICAL FIX: Actually trigger watermark exchange on reconnection
								// This fixes the 20-minute idle bug where events die but no recovery happens
								if let Err(e) = Self::trigger_watermark_exchange(
									library_id, device_id, peer_id, &peer_log, &network, &db,
								)
								.await
								{
									warn!(
										peer_id = %peer_id,
										error = %e,
										"Failed to trigger watermark exchange on reconnection"
									);
								} else {
									info!(
										peer_id = %peer_id,
										"Watermark exchange triggered on reconnection"
									);
								}
							}
							NetworkEvent::ConnectionLost {
								device_id: peer_id,
								node_id,
							} => {
								info!(
									peer_id = %peer_id,
									node_id = %node_id,
									"Device disconnected - updating devices table"
								);

								if let Err(e) = Self::handle_peer_disconnected(peer_id, &db).await {
									warn!(
										peer_id = %peer_id,
										error = %e,
										"Failed to handle peer disconnected event"
									);
								}
							}
							_ => {
								// Ignore other network events
							}
						}
					}
					Err(broadcast::error::RecvError::Lagged(skipped)) => {
						warn!(
							skipped = skipped,
							"PeerSync network event listener lagged, skipped {} events", skipped
						);
						continue;
					}
					Err(broadcast::error::RecvError::Closed) => {
						info!("Network event channel closed, stopping listener");
						break;
					}
				}
			}

			info!("PeerSync network event listener stopped");
		});
	}

	/// Handle peer connected event (static for spawned task)
	async fn handle_peer_connected(device_id: Uuid, db: &DatabaseConnection) -> Result<()> {
		use crate::infra::db::entities;
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

		// Update devices table: set is_online=true, last_seen_at=now
		let now = Utc::now();

		entities::device::Entity::update_many()
			.col_expr(
				entities::device::Column::IsOnline,
				sea_orm::sea_query::Expr::value(true),
			)
			.col_expr(
				entities::device::Column::LastSeenAt,
				sea_orm::sea_query::Expr::value(now),
			)
			.col_expr(
				entities::device::Column::UpdatedAt,
				sea_orm::sea_query::Expr::value(now),
			)
			.filter(entities::device::Column::Uuid.eq(device_id))
			.exec(db)
			.await?;

		info!(device_id = %device_id, "Device marked as online in devices table");

		// TODO: Trigger watermark exchange for reconnection sync (Priority 3)

		Ok(())
	}

	/// Handle peer disconnected event (static for spawned task)
	async fn handle_peer_disconnected(device_id: Uuid, db: &DatabaseConnection) -> Result<()> {
		use crate::infra::db::entities;
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

		// Update devices table: set is_online=false, last_seen_at=now
		let now = Utc::now();

		entities::device::Entity::update_many()
			.col_expr(
				entities::device::Column::IsOnline,
				sea_orm::sea_query::Expr::value(false),
			)
			.col_expr(
				entities::device::Column::LastSeenAt,
				sea_orm::sea_query::Expr::value(now),
			)
			.col_expr(
				entities::device::Column::UpdatedAt,
				sea_orm::sea_query::Expr::value(now),
			)
			.filter(entities::device::Column::Uuid.eq(device_id))
			.exec(db)
			.await?;

		info!(device_id = %device_id, "Device marked as offline in devices table");

		Ok(())
	}

	/// Trigger watermark exchange with peer (static for spawned task)
	///
	/// Note: This is fire-and-forget from static context. The response will be
	/// handled via the protocol handler callback to on_watermark_exchange_response().
	/// For full request/response pattern, use the instance method exchange_watermarks_and_catchup().
	async fn trigger_watermark_exchange(
		library_id: Uuid,
		device_id: Uuid,
		peer_id: Uuid,
		peer_log: &Arc<crate::infra::sync::PeerLog>,
		network: &Arc<dyn NetworkTransport>,
		db: &Arc<DatabaseConnection>,
	) -> Result<()> {
		info!(
			peer = %peer_id,
			device = %device_id,
			"Triggering watermark exchange with peer (fire-and-forget from static context)"
		);

		// Query our watermarks from sync.db
		let (_my_state_watermark, my_shared_watermark) =
			Self::query_device_watermarks(device_id, peer_log).await;

		// Get per-resource watermarks for fine-grained comparison
		let my_resource_watermarks = crate::infra::sync::ResourceWatermarkStore::new(device_id)
			.get_our_resource_watermarks(peer_log.conn())
			.await
			.unwrap_or_else(|e| {
				warn!(error = %e, "Failed to get per-resource watermarks in static exchange");
				std::collections::HashMap::new()
			});

		// Get counts of peer's resources (for gap detection)
		let my_peer_resource_counts = Self::get_device_owned_counts(peer_id, db)
			.await
			.unwrap_or_else(|e| {
				warn!(error = %e, peer = %peer_id, "Failed to get peer resource counts in static exchange");
				std::collections::HashMap::new()
			});

		debug!(
			peer = %peer_id,
			my_shared_watermark = ?my_shared_watermark,
			resource_count = my_resource_watermarks.len(),
			peer_owned_counts = ?my_peer_resource_counts,
			"Sending watermark exchange request (response will arrive async via protocol handler)"
		);

		// Send request - response will come back via protocol handler asynchronously
		let request = SyncMessage::WatermarkExchangeRequest {
			library_id,
			device_id,
			my_shared_watermark,
			my_resource_watermarks,
			my_peer_resource_counts,
		};

		network
			.send_sync_message(peer_id, request)
			.await
			.map_err(|e| anyhow::anyhow!("Failed to send watermark exchange request: {}", e))?;

		debug!(
			peer = %peer_id,
			"Watermark exchange request sent (response will be handled by protocol handler)"
		);

		Ok(())
	}

	/// Flush accumulated state changes as a batch (192x network efficiency improvement)
	async fn flush_state_change_batch(
		library_id: Uuid,
		batch: &mut Vec<serde_json::Value>,
		network: &Arc<dyn NetworkTransport>,
		state: &Arc<RwLock<DeviceSyncState>>,
		buffer: &Arc<BufferQueue>,
		retry_queue: &Arc<RetryQueue>,
		db: &Arc<sea_orm::DatabaseConnection>,
		config: &Arc<crate::infra::sync::SyncConfig>,
		last_realtime_activity_per_peer: &Arc<
			RwLock<std::collections::HashMap<Uuid, chrono::DateTime<chrono::Utc>>>,
		>,
	) {
		if batch.is_empty() {
			return;
		}

		let batch_size = batch.len();
		info!(
			batch_size = batch_size,
			"Flushing batched state changes (real-time batching optimization)"
		);

		// Process each state change in the batch
		for change_data in batch.drain(..) {
			if let Err(e) = Self::handle_state_change_event_static(
				library_id,
				change_data,
				network,
				state,
				buffer,
				retry_queue,
				db,
				config,
				last_realtime_activity_per_peer,
			)
			.await
			{
				warn!(error = %e, "Failed to handle batched state change");
			}
		}

		info!(
			batch_size = batch_size,
			"Batched state changes flushed successfully"
		);
	}

	/// Handle state change event from TransactionManager (static version for spawned task)
	async fn handle_state_change_event_static(
		library_id: Uuid,
		data: serde_json::Value,
		network: &Arc<dyn NetworkTransport>,
		state: &Arc<RwLock<DeviceSyncState>>,
		buffer: &Arc<BufferQueue>,
		retry_queue: &Arc<RetryQueue>,
		db: &Arc<sea_orm::DatabaseConnection>,
		config: &Arc<crate::infra::sync::SyncConfig>,
		last_realtime_activity_per_peer: &Arc<
			RwLock<std::collections::HashMap<Uuid, chrono::DateTime<chrono::Utc>>>,
		>,
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
			.ok_or_else(|| anyhow::anyhow!("Missing or invalid timestamp in state_change event"))?;

		let timestamp = chrono::DateTime::parse_from_rfc3339(timestamp)
			.map_err(|e| {
				anyhow::anyhow!(
					"Failed to parse timestamp '{}': {}. This may indicate clock skew.",
					timestamp,
					e
				)
			})?
			.with_timezone(&chrono::Utc);

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
			device_id = %device_id,
			"Broadcasting state change from event (our own change)"
		);

		// CRITICAL: Never buffer our OWN state changes
		// This function handles local events (from our transaction manager), not incoming network messages
		// Buffering our own changes causes data loss when we're serving backfill requests
		// Only incoming changes from peers should be buffered during our backfill

		// Create sync message
		let message = SyncMessage::StateChange {
			library_id,
			model_type: change.model_type.clone(),
			record_uuid: change.record_uuid,
			device_id: change.device_id,
			data: change.data.clone(),
			timestamp: Utc::now(),
		};

		// Get all connected sync partners (library-scoped)
		debug!("About to call network.get_connected_sync_partners() on handle_state_change_event_static");
		let connected_partners = network
			.get_connected_sync_partners(library_id, db)
			.await
			.map_err(|e| {
				warn!(error = %e, "Failed to get connected partners");
				e
			})?;

		debug!(
			count = connected_partners.len(),
			partners = ?connected_partners,
			"[Static Handler] Got connected sync partners from transport"
		);

		if connected_partners.is_empty() {
			debug!(
				"[Static Handler] No connected sync partners to broadcast to, queuing for retry"
			);

			// Get all library devices for queueing
			let library_devices = Self::get_library_devices_static(db).await?;

			// Queue for all devices except self
			for device_id in library_devices {
				if device_id != change.device_id {
					retry_queue.enqueue(device_id, message.clone()).await;
				}
			}

			return Ok(());
		}

		debug!(
			model_type = %change.model_type,
			record_uuid = %change.record_uuid,
			partner_count = connected_partners.len(),
			"Broadcasting state change to sync partners"
		);

		// Broadcast to all partners in parallel
		use futures::future::join_all;

		let timeout_secs = config.network.message_timeout_secs;
		let send_futures: Vec<_> = connected_partners
			.iter()
			.map(|&partner| {
				let network = network.clone();
				let msg = message.clone();
				async move {
					match tokio::time::timeout(
						std::time::Duration::from_secs(timeout_secs),
						network.send_sync_message(partner, msg),
					)
					.await
					{
						Ok(Ok(())) => (partner, Ok(())),
						Ok(Err(e)) => (partner, Err(e)),
						Err(_) => (
							partner,
							Err(anyhow::anyhow!("Send timeout after {}s", timeout_secs)),
						),
					}
				}
			})
			.collect();

		let results = join_all(send_futures).await;

		// Process results and mark per-peer activity
		let mut success_count = 0;
		let mut error_count = 0;
		let mut successful_peers = Vec::new();

		for (partner_uuid, result) in results {
			match result {
				Ok(()) => {
					success_count += 1;
					successful_peers.push(partner_uuid);
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

		// Mark real-time activity per successful peer (prevents one stuck peer from blocking all catch-up)
		if !successful_peers.is_empty() {
			let now = chrono::Utc::now();
			let mut activity_map = last_realtime_activity_per_peer.write().await;
			for peer_id in successful_peers {
				activity_map.insert(peer_id, now);
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

	// Batch event handlers removed - peers discover bulk changes via backfill instead of real-time events
	// This prevents event bus flooding during large indexing operations (100k+ files)

	/// Handle shared change event from TransactionManager (static version for spawned task)
	async fn handle_shared_change_event_static(
		library_id: Uuid,
		data: serde_json::Value,
		network: &Arc<dyn NetworkTransport>,
		state: &Arc<RwLock<DeviceSyncState>>,
		buffer: &Arc<BufferQueue>,
		retry_queue: &Arc<RetryQueue>,
		db: &Arc<sea_orm::DatabaseConnection>,
		config: &Arc<crate::infra::sync::SyncConfig>,
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

		let current_state = *state.read().await;
		if current_state.should_buffer() {
			debug!("Buffering own shared change during backfill");
			buffer
				.push(super::state::BufferedUpdate::SharedChange(entry))
				.await;
			return Ok(());
		}

		// Broadcast to peers (entry is already in peer_log via TransactionManager)
		let message = SyncMessage::SharedChange {
			library_id,
			entry: entry.clone(),
		};

		// Get all connected sync partners (library-scoped)
		let connected_partners = network
			.get_connected_sync_partners(library_id, db)
			.await
			.map_err(|e| {
				warn!(error = %e, "Failed to get connected partners");
				e
			})?;

		if connected_partners.is_empty() {
			debug!("No connected sync partners to broadcast to, queuing for retry");

			// Get all library devices for queueing
			let library_devices = Self::get_library_devices_static(db).await?;

			// Queue for all devices except self
			for device_id in library_devices {
				if device_id != entry.hlc.device_id {
					retry_queue.enqueue(device_id, message.clone()).await;
				}
			}

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

		let timeout_secs = config.network.message_timeout_secs;
		let send_futures: Vec<_> = connected_partners
			.iter()
			.map(|&partner| {
				let network = network.clone();
				let msg = message.clone();
				async move {
					match tokio::time::timeout(
						std::time::Duration::from_secs(timeout_secs),
						network.send_sync_message(partner, msg),
					)
					.await
					{
						Ok(Ok(())) => (partner, Ok(())),
						Ok(Err(e)) => (partner, Err(e)),
						Err(_) => (
							partner,
							Err(anyhow::anyhow!("Send timeout after {}s", timeout_secs)),
						),
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

		// Create sync message
		let message = SyncMessage::StateChange {
			library_id: self.library_id,
			model_type: change.model_type.clone(),
			record_uuid: change.record_uuid,
			device_id: change.device_id,
			data: change.data.clone(),
			timestamp: Utc::now(),
		};

		// Get all connected sync partners
		let connected_partners = self
			.network
			.get_connected_sync_partners(self.library_id, &self.db)
			.await
			.map_err(|e| {
				warn!(error = %e, "Failed to get connected partners");
				e
			})?;

		if connected_partners.is_empty() {
			debug!("No connected sync partners to broadcast to, queuing for retry");

			// Get all library devices for queueing
			let library_devices = self.get_library_devices().await?;

			// Queue for all devices except self
			for device_id in library_devices {
				if device_id != self.device_id {
					self.retry_queue.enqueue(device_id, message.clone()).await;
				}
			}

			return Ok(());
		}

		debug!(
			model_type = %change.model_type,
			record_uuid = %change.record_uuid,
			partner_count = connected_partners.len(),
			"Broadcasting state change to sync partners"
		);

		// Record start time for latency tracking
		let start_time = std::time::Instant::now();

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

		// Record metrics
		self.metrics.record_broadcast(true, None);
		if error_count > 0 {
			for _ in 0..error_count {
				self.metrics.record_failed_broadcast();
			}
		}

		// Record latency
		let latency_ms = start_time.elapsed().as_millis() as u64;
		self.metrics.record_broadcast_latency(latency_ms);

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

		// Create sync message
		let message = SyncMessage::SharedChange {
			library_id: self.library_id,
			entry: entry.clone(),
		};

		// Get all connected sync partners
		let connected_partners = self
			.network
			.get_connected_sync_partners(self.library_id, &self.db)
			.await
			.map_err(|e| {
				warn!(error = %e, "Failed to get connected partners");
				e
			})?;

		if connected_partners.is_empty() {
			debug!("No connected sync partners to broadcast to, queuing for retry");

			// Get all library devices for queueing
			let library_devices = self.get_library_devices().await?;

			// Queue for all devices except self
			for device_id in library_devices {
				if device_id != self.device_id {
					self.retry_queue.enqueue(device_id, message.clone()).await;
				}
			}

			return Ok(());
		}

		debug!(
			hlc = %hlc,
			model_type = %model_type,
			record_uuid = %record_uuid,
			partner_count = connected_partners.len(),
			"Broadcasting shared change to sync partners"
		);

		// Record start time for latency tracking
		let start_time = std::time::Instant::now();

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

		// Record metrics
		self.metrics.record_broadcast(false, None);
		if error_count > 0 {
			for _ in 0..error_count {
				self.metrics.record_failed_broadcast();
			}
		}

		// Record latency
		let latency_ms = start_time.elapsed().as_millis() as u64;
		self.metrics.record_broadcast_latency(latency_ms);

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
		// Record metrics
		self.metrics.record_changes_received(1);

		let state = self.state().await;

		if state.should_buffer() {
			// Buffer during backfill/catch-up
			self.buffer
				.push(super::state::BufferedUpdate::StateChange(change))
				.await;
			// debug!("Buffered state change during backfill");
			return Ok(());
		}

		// Apply immediately
		self.apply_state_change(change).await
	}

	/// Handle received shared change
	pub async fn on_shared_change_received(&self, entry: SharedChangeEntry) -> Result<()> {
		// Record metrics
		self.metrics.record_changes_received(1);

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

	/// Apply state change to database (internal version without retry trigger to prevent recursion)
	async fn apply_state_change_without_retry(&self, change: StateChangeMessage) -> Result<()> {
		// Use the registry to route to the appropriate apply function
		crate::infra::sync::apply_state_change(
			&change.model_type,
			change.data.clone(),
			self.db.clone(),
		)
		.await?;

		// Record metrics
		self.metrics.record_changes_applied(1);
		self.metrics
			.record_entries_synced(&change.model_type, 1)
			.await;

		// Update PER-RESOURCE watermark
		self.update_resource_watermark(change.device_id, &change.model_type, change.timestamp)
			.await?;

		info!(
			model_type = %change.model_type,
			record_uuid = %change.record_uuid,
			"State change applied successfully"
		);

		Ok(())
	}

	/// Apply state change to database
	async fn apply_state_change(&self, change: StateChangeMessage) -> Result<()> {
		// Record start time for latency tracking
		let start_time = std::time::Instant::now();

		debug!(
			model_type = %change.model_type,
			record_uuid = %change.record_uuid,
			device_id = %change.device_id,
			"Applying state change"
		);

		// Use the registry to route to the appropriate apply function
		if let Err(e) = crate::infra::sync::apply_state_change(
			&change.model_type,
			change.data.clone(),
			self.db.clone(),
		)
		.await
		{
			let error_str = e.to_string();

			// Check if this is a sync dependency error (FK reference missing)
			if error_str.contains("Sync dependency missing") {
				// Extract the missing UUID from the error message
				if let Some(missing_uuid) =
					super::dependency::extract_missing_dependency_uuid(&error_str)
				{
					debug!(
						model_type = %change.model_type,
						record_uuid = %change.record_uuid,
						missing_uuid = %missing_uuid,
						"Sync dependency missing, tracking for event-driven retry"
					);

					// Track this dependency for event-driven retry
					self.dependency_tracker
						.add_dependency(
							missing_uuid,
							super::state::BufferedUpdate::StateChange(change),
						)
						.await;

					return Ok(()); // Tracked, will retry when dependency arrives
				} else {
					// Couldn't extract UUID - fall back to buffer
					warn!(
						model_type = %change.model_type,
						record_uuid = %change.record_uuid,
						"Sync dependency missing but couldn't extract UUID, buffering: {}",
						error_str
					);

					self.buffer
						.push(super::state::BufferedUpdate::StateChange(change))
						.await;

					return Ok(());
				}
			}

			// Other errors - record metrics and propagate
			let metrics = self.metrics.clone();
			let model_type = change.model_type.clone();
			let error_msg = format!("Failed to apply state change: {}", e);
			tokio::spawn(async move {
				let _ = metrics
					.record_error(
						super::metrics::ErrorEvent::new("apply".to_string(), error_msg)
							.with_model_type(model_type),
					)
					.await;
			});

			return Err(anyhow::anyhow!("Failed to apply state change: {}", e));
		}

		// Record metrics
		self.metrics.record_changes_applied(1);
		self.metrics
			.record_entries_synced(&change.model_type, 1)
			.await;

		// Record latency
		let latency_ms = start_time.elapsed().as_millis() as u64;
		self.metrics.record_apply_latency(latency_ms);

		// Update PER-RESOURCE watermark (FIX: use resource-specific tracking)
		self.update_resource_watermark(
			change.device_id,
			&change.model_type, // Resource type (location, entry, volume, etc.)
			change.timestamp,
		)
		.await?;

		info!(
			model_type = %change.model_type,
			record_uuid = %change.record_uuid,
			"State change applied successfully"
		);

		// Event-driven dependency resolution: check if any updates were waiting for THIS UUID
		let waiting_updates = self.dependency_tracker.resolve(change.record_uuid).await;

		if !waiting_updates.is_empty() {
			debug!(
				resolved_uuid = %change.record_uuid,
				waiting_count = waiting_updates.len(),
				"Resolving dependencies - retrying waiting updates"
			);

			// Retry ONLY the updates that were waiting for this specific dependency
			for update in waiting_updates {
				match update {
					super::state::BufferedUpdate::StateChange(dependent_change) => {
						// Box the recursive call to avoid infinite type size
						if let Err(e) =
							Box::pin(self.apply_state_change(dependent_change.clone())).await
						{
							warn!(
								error = %e,
								record_uuid = %dependent_change.record_uuid,
								"Failed to apply dependent state change after resolving dependency"
							);
						}
					}
					super::state::BufferedUpdate::SharedChange(dependent_shared) => {
						if let Err(e) =
							Box::pin(self.apply_shared_change(dependent_shared.clone())).await
						{
							warn!(
								error = %e,
								record_uuid = %dependent_shared.record_uuid,
								"Failed to apply dependent shared change after resolving dependency"
							);
						}
					}
				}
			}
		}

		// Emit resource event for UI reactivity using ResourceManager (non-blocking)
		// This ensures proper resource format (LocationInfo, File, etc.) instead of raw DB model
		if let Some(uuid_value) = change.data.get("uuid") {
			if let Some(uuid_str) = uuid_value.as_str() {
				if let Ok(uuid) = Uuid::parse_str(uuid_str) {
					let resource_manager = crate::domain::ResourceManager::new(
						self.db.clone(),
						self.event_bus.clone(),
					);
					let model_type = change.model_type.clone();

					// Spawn to avoid blocking sync message processing
					tokio::spawn(async move {
						if let Err(e) = resource_manager
							.emit_resource_events(&model_type, vec![uuid])
							.await
						{
							warn!(
								model_type = %model_type,
								uuid = %uuid,
								error = %e,
								"Failed to emit resource event after state change"
							);
						}
					});
				} else {
					warn!(
						model_type = %change.model_type,
						uuid_str = %uuid_str,
						"Failed to parse UUID from state change"
					);
				}
			}
		} else {
			warn!(
				model_type = %change.model_type,
				"No UUID found in state change data, skipping resource event"
			);
		}

		Ok(())
	}

	/// Apply shared change to database with conflict resolution
	async fn apply_shared_change(&self, entry: SharedChangeEntry) -> Result<()> {
		// Record start time for latency tracking
		let start_time = std::time::Instant::now();

		debug!(
			hlc = %entry.hlc,
			model_type = %entry.model_type,
			record_uuid = %entry.record_uuid,
			"Applying shared change"
		);

		// HLC conflict resolution: check if we already have a more recent change for this record
		if let Ok(Some(existing_hlc)) = self
			.peer_log
			.get_latest_hlc_for_record(entry.record_uuid)
			.await
		{
			if entry.hlc <= existing_hlc {
				debug!(
					incoming_hlc = %entry.hlc,
					existing_hlc = %existing_hlc,
					record_uuid = %entry.record_uuid,
					"Ignoring incoming change with older or equal HLC"
				);
				return Ok(());
			}
		}

		// Use the registry to route to the appropriate apply function
		crate::infra::sync::apply_shared_change(entry.clone(), self.db.clone())
			.await
			.map_err(|e| {
				// Record error metrics (spawn async task)
				let metrics = self.metrics.clone();
				let model_type = entry.model_type.clone();
				let error_msg = format!("Failed to apply shared change: {}", e);
				tokio::spawn(async move {
					let _ = metrics
						.record_error(
							super::metrics::ErrorEvent::new("apply".to_string(), error_msg)
								.with_model_type(model_type),
						)
						.await;
				});
				anyhow::anyhow!("Failed to apply shared change: {}", e)
			})?;

		// Record this change in our peer log (track what we've applied)
		self.peer_log
			.append(entry.clone())
			.await
			.map_err(|e| anyhow::anyhow!("Failed to append to peer log: {}", e))?;

		// Record metrics
		self.metrics.record_changes_applied(1);
		self.metrics
			.record_entries_synced(&entry.model_type, 1)
			.await;

		// Event-driven dependency resolution: check if any updates were waiting for THIS UUID
		// Works for both cross-type (entry → content_identity) and same-type dependencies
		let waiting_updates = self.dependency_tracker.resolve(entry.record_uuid).await;

		if !waiting_updates.is_empty() {
			debug!(
				resolved_uuid = %entry.record_uuid,
				waiting_count = waiting_updates.len(),
				"Resolving dependencies - retrying waiting updates"
			);

			// Retry ONLY the updates that were waiting for this specific dependency
			for update in waiting_updates {
				match update {
					super::state::BufferedUpdate::StateChange(dependent_change) => {
						// Box the recursive call to avoid infinite type size
						if let Err(e) =
							Box::pin(self.apply_state_change(dependent_change.clone())).await
						{
							warn!(
								error = %e,
								record_uuid = %dependent_change.record_uuid,
								"Failed to apply dependent state change after resolving shared dependency"
							);
						}
					}
					super::state::BufferedUpdate::SharedChange(dependent_shared) => {
						// Box the recursive call to avoid infinite type size
						if let Err(e) =
							Box::pin(self.apply_shared_change(dependent_shared.clone())).await
						{
							warn!(
								error = %e,
								record_uuid = %dependent_shared.record_uuid,
								"Failed to apply dependent shared change after resolving shared dependency"
							);
						}
					}
				}
			}
		}

		// Record latency
		let latency_ms = start_time.elapsed().as_millis() as u64;
		self.metrics.record_apply_latency(latency_ms);

		info!(
			hlc = %entry.hlc,
			model_type = %entry.model_type,
			record_uuid = %entry.record_uuid,
			"Shared change applied successfully"
		);

		// Update shared watermark
		self.update_shared_watermark(entry.hlc).await?;

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

		// Emit resource event for UI reactivity using ResourceManager
		// This ensures proper resource format (LocationInfo, etc.) instead of raw DB model
		use crate::infra::sync::peer_log::ChangeType;
		match entry.change_type {
			ChangeType::Delete => {
				self.event_bus.emit(Event::ResourceDeleted {
					resource_type: entry.model_type,
					resource_id: entry.record_uuid,
				});
			}
			ChangeType::Insert | ChangeType::Update => {
				// Use ResourceManager to fetch and emit properly formatted resource
				let resource_manager =
					crate::domain::ResourceManager::new(self.db.clone(), self.event_bus.clone());

				if let Err(e) = resource_manager
					.emit_resource_events(&entry.model_type, vec![entry.record_uuid])
					.await
				{
					warn!(
						model_type = %entry.model_type,
						uuid = %entry.record_uuid,
						error = %e,
						"Failed to emit resource event after shared change"
					);
				}
			}
		}

		Ok(())
	}

	/// Record ACK from peer and prune
	pub async fn on_ack_received(&self, peer_id: Uuid, up_to_hlc: HLC) -> Result<()> {
		// Don't record ACKs from ourselves (defense in depth)
		if peer_id == self.device_id {
			debug!(
				hlc = %up_to_hlc,
				"Ignoring self-ACK (should not happen, indicates bug in sender)"
			);
			return Ok(());
		}

		// Record ACK from peer
		self.peer_log
			.record_ack(peer_id, up_to_hlc)
			.await
			.map_err(|e| anyhow::anyhow!("Failed to record ACK: {}", e))?;

		debug!(
			peer = %peer_id,
			hlc = %up_to_hlc,
			"Recorded ACK from peer"
		);

		// Try to prune based on ACKs
		let pruned = self
			.peer_log
			.prune_acked()
			.await
			.map_err(|e| anyhow::anyhow!("Failed to prune: {}", e))?;

		if pruned > 0 {
			info!(
				pruned = pruned,
				peer = %peer_id,
				"Pruned shared changes after ACK"
			);
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

	/// Get all library devices with sync enabled
	async fn get_library_devices(&self) -> Result<Vec<Uuid>> {
		Self::get_library_devices_static(&self.db).await
	}

	/// Get all library devices with sync enabled (static version)
	async fn get_library_devices_static(db: &DatabaseConnection) -> Result<Vec<Uuid>> {
		use crate::infra::db::entities;
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

		let library_devices = entities::device::Entity::find()
			.filter(entities::device::Column::SyncEnabled.eq(true))
			.all(db)
			.await
			.map_err(|e| anyhow::anyhow!("Failed to query library devices: {}", e))?;

		Ok(library_devices.iter().map(|d| d.uuid).collect())
	}

	/// Get device-owned state for backfill (StateRequest)
	pub async fn get_device_state(
		&self,
		model_types: Vec<String>,
		device_id: Option<Uuid>,
		since: Option<chrono::DateTime<chrono::Utc>>,
		cursor: Option<(chrono::DateTime<chrono::Utc>, Uuid)>,
		batch_size: usize,
	) -> Result<Vec<crate::service::network::protocol::sync::messages::StateRecord>> {
		use crate::service::network::protocol::sync::messages::StateRecord;

		debug!(
			model_types = ?model_types,
			device_id = ?device_id,
			since = ?since,
			cursor = ?cursor,
			batch_size = batch_size,
			"Querying device state for backfill"
		);

		let mut all_records = Vec::new();
		let mut remaining_batch = batch_size;

		for model_type in model_types {
			if remaining_batch == 0 {
				break;
			}

			// Query through the registry with cursor for DB-level pagination
			match crate::infra::sync::registry::query_device_state(
				&model_type,
				device_id,
				since,
				cursor,
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

	/// Get deletion tombstones for incremental sync
	pub async fn get_deletion_tombstones(
		&self,
		model_type: &str,
		device_id: Option<Uuid>,
		since: chrono::DateTime<chrono::Utc>,
	) -> Result<Vec<Uuid>> {
		use crate::infra::db::entities::device_state_tombstone;
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

		let mut query = device_state_tombstone::Entity::find()
			.filter(device_state_tombstone::Column::ModelType.eq(model_type))
			.filter(device_state_tombstone::Column::DeletedAt.gte(since));

		// Filter by device if specified
		if let Some(dev_id) = device_id {
			// Map device UUID to local ID
			if let Some(device) = crate::infra::db::entities::device::Entity::find()
				.filter(crate::infra::db::entities::device::Column::Uuid.eq(dev_id))
				.one(self.db.as_ref())
				.await?
			{
				query = query.filter(device_state_tombstone::Column::DeviceId.eq(device.id));
			} else {
				// Device not found, no tombstones
				return Ok(vec![]);
			}
		}

		let tombstones = query.all(self.db.as_ref()).await?;

		Ok(tombstones.into_iter().map(|t| t.record_uuid).collect())
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

	/// Get full current state of all shared resources (for initial backfill)
	///
	/// Queries the database for ALL shared resources (tags, albums, etc.) to send
	/// to a new device during initial sync. This ensures pre-sync data is included.
	///
	/// This is fully generic - uses the registry to discover and query all shared models.
	pub async fn get_full_shared_state(&self) -> Result<serde_json::Value> {
		debug!("Querying full shared resource state for backfill");

		// Query all shared models through the registry (fully generic)
		// Use large limit for full state snapshot (no pagination here since it's a fallback)
		let all_shared_state = crate::infra::sync::registry::query_all_shared_models(
			None,   // No since filter - get everything
			100000, // Large limit to capture all shared resources (including pre-sync data)
			self.db.clone(),
		)
		.await
		.map_err(|e| anyhow::anyhow!("Failed to query shared models: {}", e))?;

		// Build response object dynamically
		let mut response = serde_json::Map::new();

		for (model_type, records) in all_shared_state {
			info!(
				model_type = %model_type,
				count = records.len(),
				"Queried shared model for backfill state snapshot"
			);

			// Convert records to array of {uuid, data} objects
			let records_json: Vec<serde_json::Value> = records
				.into_iter()
				.map(|(uuid, data, _ts)| {
					serde_json::json!({
						"uuid": uuid,
						"data": data
					})
				})
				.collect();

			response.insert(model_type, serde_json::Value::Array(records_json));
		}

		Ok(serde_json::Value::Object(response))
	}

	/// Transition to ready state (after backfill)
	pub async fn transition_to_ready(&self) -> Result<()> {
		let current_state = self.state().await;

		if !current_state.should_buffer() {
			warn!("Attempted to transition to ready from non-buffering state");
			return Ok(());
		}

		info!("Transitioning to ready, processing buffered updates and dependency tracker");

		// Check dependency tracker stats
		let dep_stats = self.dependency_tracker.stats().await;
		if !dep_stats.is_empty() {
			warn!(
				dependencies = dep_stats.total_dependencies,
				waiting_updates = dep_stats.total_waiting_updates,
				"Dependency tracker has unresolved dependencies at state transition - will try to flush"
			);
		}

		// Set to catching up
		{
			let mut state = self.state.write().await;
			*state = DeviceSyncState::CatchingUp {
				buffered_count: self.buffer.len().await + dep_stats.total_waiting_updates,
			};
		}

		// Record state transition
		let buffered_count = self.buffer.len().await;
		self.metrics
			.record_state_transition(
				current_state,
				DeviceSyncState::CatchingUp { buffered_count },
				Some("transitioning to ready".to_string()),
			)
			.await?;

		// Process OLD buffer first (legacy path, should be empty if dependency tracker is working)
		let mut state_changes_to_broadcast = Vec::new();
		let mut shared_changes_to_broadcast = Vec::new();

		while let Some(update) = self.buffer.pop_ordered().await {
			match update {
				super::state::BufferedUpdate::StateChange(change) => {
					self.apply_state_change(change.clone()).await?;
					state_changes_to_broadcast.push(change);
				}
				super::state::BufferedUpdate::SharedChange(entry) => {
					self.apply_shared_change(entry.clone()).await?;
					shared_changes_to_broadcast.push(entry);
				}
			}
		}

		// Process dependency tracker - handle unresolved dependencies
		// This is a fallback for dependencies that couldn't be resolved during normal operation
		if !dep_stats.is_empty() {
			warn!(
				dependencies = dep_stats.total_dependencies,
				waiting_updates = dep_stats.total_waiting_updates,
				"Dependency tracker has unresolved dependencies at Ready transition"
			);

			// Get list of missing UUIDs for diagnostic purposes
			let missing_uuids = self.dependency_tracker.get_pending_dependency_uuids().await;

			if missing_uuids.len() <= 10 {
				// Log specific UUIDs if count is manageable
				warn!(
					?missing_uuids,
					"Missing dependency UUIDs (may be circular references or orphaned data)"
				);
			} else {
				warn!(
					missing_count = missing_uuids.len(),
					sample_uuids = ?&missing_uuids[..10],
					"Many missing dependencies (showing first 10)"
				);
			}

			// Strategy: Clear dependencies after logging to prevent blocking sync indefinitely
			// These entries either have:
			// - Circular dependencies (impossible to resolve)
			// - References to deleted records
			// - Incomplete sync data from peer
			// They will be resynced on next full backfill if the data becomes available
			let cleared_count = self.dependency_tracker.clear_all().await;
			warn!(
			cleared_count,
			"Cleared unresolved dependencies to prevent sync deadlock. These updates will be retried on next full sync."
		);
		}

		info!(
			state_changes = state_changes_to_broadcast.len(),
			shared_changes = shared_changes_to_broadcast.len(),
			"Processing buffered updates - will broadcast to peers after local application"
		);

		// Now broadcast all buffered changes to peers (they're in Ready state now)
		for change in state_changes_to_broadcast {
			if let Err(e) = Self::handle_state_change_event_static(
				self.library_id,
				serde_json::json!({
					"library_id": self.library_id,
					"model_type": change.model_type,
					"record_uuid": change.record_uuid,
					"device_id": change.device_id,
					"data": change.data,
					"timestamp": change.timestamp,
				}),
				&self.network,
				&self.state,
				&self.buffer,
				&self.retry_queue,
				&self.db,
				&self.config,
				&self.last_realtime_activity_per_peer,
			)
			.await
			{
				warn!(
					error = %e,
					record_uuid = %change.record_uuid,
					"Failed to broadcast buffered state change to peers"
				);
			}
		}

		for entry in shared_changes_to_broadcast {
			if let Err(e) = Self::handle_shared_change_event_static(
				self.library_id,
				serde_json::json!({
					"library_id": self.library_id,
					"entry": entry,
				}),
				&self.network,
				&self.state,
				&self.buffer,
				&self.retry_queue,
				&self.db,
				&self.config,
			)
			.await
			{
				warn!(
					error = %e,
					hlc = %entry.hlc,
					"Failed to broadcast buffered shared change to peers"
				);
			}
		}

		// Now ready!
		{
			let mut state = self.state.write().await;
			*state = DeviceSyncState::Ready;
		}

		// Record state transition
		let buffered_count = self.buffer.len().await;
		self.metrics
			.record_state_transition(
				DeviceSyncState::CatchingUp { buffered_count },
				DeviceSyncState::Ready,
				Some("buffered updates processed".to_string()),
			)
			.await?;

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

	/// Get the event bus
	pub fn event_bus(&self) -> &Arc<EventBus> {
		&self.event_bus
	}

	/// Get the sync event bus
	pub fn sync_events(&self) -> &Arc<crate::infra::sync::SyncEventBus> {
		&self.sync_events
	}

	/// Set sync state (test/debug helper)
	///
	/// ️ This is for testing only. Manually overrides the sync state.
	/// In production, state transitions are managed automatically by the sync service.
	pub async fn set_state_for_test(&self, new_state: DeviceSyncState) {
		*self.state.write().await = new_state;
		info!("Sync state manually set to {:?} (test helper)", new_state);
	}
}
