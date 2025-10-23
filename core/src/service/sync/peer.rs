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
use tokio::sync::{broadcast, mpsc, RwLock};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use super::{
	retry_queue::RetryQueue,
	state::{BufferQueue, DeviceSyncState, StateChangeMessage},
};

/// Timeout for sending sync messages to peers
///
/// Messages taking longer than this are cancelled to prevent blocking
/// on slow or unresponsive devices.
const SYNC_MESSAGE_TIMEOUT_SECS: u64 = 30;

/// Interval for processing retry queue
///
/// Failed messages are retried every 10 seconds with exponential backoff.
const RETRY_PROCESSOR_INTERVAL_SECS: u64 = 10;

/// Interval for pruning acknowledged log entries
///
/// Runs every 5 minutes to clean up peer log entries that all devices
/// have acknowledged receiving.
const LOG_PRUNER_INTERVAL_SECS: u64 = 300;

/// Main sync loop interval
///
/// Checks sync state and performs maintenance every 5 seconds.
const SYNC_LOOP_INTERVAL_SECS: u64 = 5;

/// Default batch size for catch-up requests
///
/// Used when requesting missing changes from peers during reconnection.
/// Same as backfill batch size for consistency.
const CATCHUP_BATCH_SIZE: usize = 10_000;

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

	/// Network event receiver (optional - if provided, enables connection event handling)
	network_events: Arc<
		tokio::sync::Mutex<
			Option<broadcast::Receiver<crate::service::network::core::NetworkEvent>>,
		>,
	>,
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
			network_events: Arc::new(tokio::sync::Mutex::new(None)),
		})
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

	/// Query watermarks from devices table (shared helper)
	async fn query_device_watermarks(
		device_id: Uuid,
		db: &DatabaseConnection,
	) -> (Option<chrono::DateTime<chrono::Utc>>, Option<HLC>) {
		use crate::infra::db::entities;
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

		match entities::device::Entity::find()
			.filter(entities::device::Column::Uuid.eq(device_id))
			.one(db)
			.await
		{
			Ok(Some(device)) => {
				let state_watermark = device.last_state_watermark;

				// Deserialize shared watermark from JSON
				let shared_watermark = device
					.last_shared_watermark
					.as_ref()
					.and_then(|json_str| serde_json::from_str(json_str).ok());

				(state_watermark, shared_watermark)
			}
			Ok(None) => {
				warn!(
					device_id = %device_id,
					"Device not found in devices table, returning None watermarks"
				);
				(None, None)
			}
			Err(e) => {
				warn!(
					device_id = %device_id,
					error = %e,
					"Failed to query watermarks from devices table"
				);
				(None, None)
			}
		}
	}

	/// Get watermarks for heartbeat and reconnection sync
	///
	/// Returns (state_watermark, shared_watermark) from the devices table.
	/// State watermark tracks device-owned data (locations, entries).
	/// Shared watermark (HLC) tracks shared resources (tags, albums).
	pub async fn get_watermarks(&self) -> (Option<chrono::DateTime<chrono::Utc>>, Option<HLC>) {
		Self::query_device_watermarks(self.device_id, self.db.as_ref()).await
	}

	/// Update state watermark for a device after processing state changes
	async fn update_state_watermark(&self, device_id: Uuid, timestamp: chrono::DateTime<chrono::Utc>) -> Result<()> {
		use crate::infra::db::entities;
		use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};

		// Find device and update watermark
		let device = entities::device::Entity::find()
			.filter(entities::device::Column::Uuid.eq(device_id))
			.one(self.db.as_ref())
			.await
			.map_err(|e| anyhow::anyhow!("Failed to query device: {}", e))?
			.ok_or_else(|| anyhow::anyhow!("Device not found: {}", device_id))?;

		let mut device_active: entities::device::ActiveModel = device.into();

		// Update watermark if this timestamp is newer
		match &device_active.last_state_watermark {
			Set(Some(current)) | sea_orm::ActiveValue::Unchanged(Some(current)) if timestamp <= *current => {
				// Don't update if we already have a newer watermark
				return Ok(());
			}
			_ => {
				device_active.last_state_watermark = Set(Some(timestamp));
			}
		}

		device_active.update(self.db.as_ref()).await
			.map_err(|e| anyhow::anyhow!("Failed to update state watermark: {}", e))?;

		debug!("Updated state watermark for device {} to {}", device_id, timestamp);
		Ok(())
	}

	/// Update shared watermark for this device after processing shared changes
	async fn update_shared_watermark(&self, hlc: HLC) -> Result<()> {
		use crate::infra::db::entities;
		use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};

		// Find device and update watermark
		let device = entities::device::Entity::find()
			.filter(entities::device::Column::Uuid.eq(self.device_id))
			.one(self.db.as_ref())
			.await
			.map_err(|e| anyhow::anyhow!("Failed to query device: {}", e))?
			.ok_or_else(|| anyhow::anyhow!("Device not found: {}", self.device_id))?;

		let mut device_active: entities::device::ActiveModel = device.into();

		// Serialize HLC to JSON
		let hlc_json = serde_json::to_string(&hlc)
			.map_err(|e| anyhow::anyhow!("Failed to serialize HLC: {}", e))?;

		// Update watermark if this HLC is newer
		let should_update = match &device_active.last_shared_watermark {
			Set(Some(current_json)) | sea_orm::ActiveValue::Unchanged(Some(current_json)) => {
				if let Ok(current_hlc) = serde_json::from_str::<HLC>(current_json) {
					hlc > current_hlc
				} else {
					true  // Invalid JSON, update anyway
				}
			}
			_ => true  // No watermark set yet
		};

		if should_update {
			device_active.last_shared_watermark = Set(Some(hlc_json));
			device_active.update(self.db.as_ref()).await
				.map_err(|e| anyhow::anyhow!("Failed to update shared watermark: {}", e))?;
			debug!("Updated shared watermark for device {} to {}", self.device_id, hlc);
		}

		Ok(())
	}

	/// Set initial watermarks after backfill (called by backfill manager)
	///
	/// Uses actual checkpoints from received data instead of querying local database
	/// This prevents watermark drift due to clock skew between devices
	pub async fn set_initial_watermarks(
		&self,
		final_state_checkpoint: Option<String>,
		max_shared_hlc: Option<crate::infra::sync::HLC>,
	) -> Result<()> {
		use crate::infra::db::entities;
		use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

		let now = chrono::Utc::now();

		// Parse state checkpoint (timestamp|uuid format) to extract timestamp
		// Use the timestamp from the last record RECEIVED, not from our local database
		let state_watermark = if let Some(checkpoint) = final_state_checkpoint {
			let ts_str = checkpoint.split('|').next().unwrap_or("");
			chrono::DateTime::parse_from_rfc3339(ts_str)
				.ok()
				.map(|dt| dt.with_timezone(&chrono::Utc))
				.unwrap_or(now)
		} else {
			// No data received, use current time
			now
		};

		// Use the max HLC from received shared data, or generate new one if no data received
		let shared_watermark_hlc = max_shared_hlc.unwrap_or_else(|| {
			// No shared data received, generate current HLC as baseline
			futures::executor::block_on(async {
				self.hlc_generator.lock().await.next()
			})
		});

		// Update this device's watermarks
		let device = entities::device::Entity::find()
			.filter(entities::device::Column::Uuid.eq(self.device_id))
			.one(self.db.as_ref())
			.await
			.map_err(|e| anyhow::anyhow!("Failed to query device: {}", e))?
			.ok_or_else(|| anyhow::anyhow!("Device not found: {}", self.device_id))?;

		let mut device_active: entities::device::ActiveModel = device.into();
		device_active.last_state_watermark = Set(Some(state_watermark));
		device_active.last_shared_watermark = Set(Some(
			serde_json::to_string(&shared_watermark_hlc).unwrap_or_default()
		));
		device_active.last_sync_at = Set(Some(now));

		device_active.update(self.db.as_ref()).await
			.map_err(|e| anyhow::anyhow!("Failed to set initial watermarks: {}", e))?;

		info!(
			"Set watermarks from received data: state={} (from checkpoint), shared={} (from max HLC), last_sync_at={}",
			state_watermark, shared_watermark_hlc, now
		);

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
		let (my_state_watermark, my_shared_watermark) = self.get_watermarks().await;

		debug!(
			peer = %peer_id,
			my_state_watermark = ?my_state_watermark,
			my_shared_watermark = ?my_shared_watermark,
			"Sending watermark exchange request"
		);

		// Send request to peer
		let request = SyncMessage::WatermarkExchangeRequest {
			library_id: self.library_id,
			device_id: self.device_id,
			my_state_watermark,
			my_shared_watermark,
		};

		self.network
			.send_sync_message(peer_id, request)
			.await
			.map_err(|e| anyhow::anyhow!("Failed to send watermark exchange request: {}", e))?;

		info!(
			peer = %peer_id,
			"Watermark exchange request sent, waiting for response"
		);

		Ok(())
	}

	/// Handle watermark exchange response from peer
	///
	/// Called by the protocol handler when a WatermarkExchangeResponse is received.
	/// Triggers incremental catch-up if watermarks diverge.
	pub async fn on_watermark_exchange_response(
		&self,
		peer_id: Uuid,
		peer_state_watermark: Option<chrono::DateTime<chrono::Utc>>,
		peer_shared_watermark: Option<HLC>,
		needs_state_catchup: bool,
		needs_shared_catchup: bool,
	) -> Result<()> {
		info!(
			peer = %peer_id,
			peer_state_watermark = ?peer_state_watermark,
			peer_shared_watermark = ?peer_shared_watermark,
			needs_state_catchup = needs_state_catchup,
			needs_shared_catchup = needs_shared_catchup,
			"Received watermark exchange response"
		);

		// Get our watermarks to compare
		let (my_state_watermark, my_shared_watermark) = self.get_watermarks().await;

		// Determine if WE need to catch up based on watermark comparison
		let mut we_need_state_catchup = false;
		let mut we_need_shared_catchup = false;

		// Compare state watermarks (timestamps)
		match (my_state_watermark, peer_state_watermark) {
			(Some(my_ts), Some(peer_ts)) if peer_ts > my_ts => {
				info!(
					peer = %peer_id,
					my_timestamp = %my_ts,
					peer_timestamp = %peer_ts,
					"Peer has newer state, need to catch up"
				);
				we_need_state_catchup = true;
			}
			(None, Some(_)) => {
				info!(peer = %peer_id, "We have no state watermark, need full state catch-up");
				we_need_state_catchup = true;
			}
			_ => {
				debug!(peer = %peer_id, "State watermarks in sync");
			}
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
			info!(peer = %peer_id, "Requesting incremental state catch-up");
			// TODO: Implement incremental state request
			// For now, log that full backfill will occur
			warn!(
				peer = %peer_id,
				"Incremental state catch-up not yet implemented, will use backfill"
			);
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
				limit: CATCHUP_BATCH_SIZE,
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
		self.update_peer_watermarks(peer_id, peer_state_watermark, peer_shared_watermark)
			.await?;

		info!(peer = %peer_id, "Watermark exchange complete");

		Ok(())
	}

	/// Update peer's watermarks in the devices table
	async fn update_peer_watermarks(
		&self,
		peer_id: Uuid,
		state_watermark: Option<chrono::DateTime<chrono::Utc>>,
		shared_watermark: Option<HLC>,
	) -> Result<()> {
		use crate::infra::db::entities;
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

		// Serialize shared watermark to JSON
		let shared_watermark_json = shared_watermark
			.map(|hlc| serde_json::to_string(&hlc).ok())
			.flatten();

		entities::device::Entity::update_many()
			.col_expr(
				entities::device::Column::LastStateWatermark,
				sea_orm::sea_query::Expr::value(state_watermark),
			)
			.col_expr(
				entities::device::Column::LastSharedWatermark,
				sea_orm::sea_query::Expr::value(shared_watermark_json),
			)
			.col_expr(
				entities::device::Column::UpdatedAt,
				sea_orm::sea_query::Expr::value(Utc::now()),
			)
			.filter(entities::device::Column::Uuid.eq(peer_id))
			.exec(self.db.as_ref())
			.await?;

		debug!(
			peer = %peer_id,
			state_watermark = ?state_watermark,
			shared_watermark = ?shared_watermark,
			"Updated peer watermarks in devices table"
		);

		Ok(())
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

		// Start network event listener for connection tracking
		self.start_network_event_listener().await;

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
				// Check for ready messages at configured interval
				tokio::time::sleep(tokio::time::Duration::from_secs(
					RETRY_PROCESSOR_INTERVAL_SECS,
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

		tokio::spawn(async move {
			info!("Started log pruner");

			while is_running.load(Ordering::SeqCst) {
				// Prune at configured interval
				tokio::time::sleep(tokio::time::Duration::from_secs(LOG_PRUNER_INTERVAL_SECS))
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
						info!("PeerSync received event: {}", event_type);
						match event_type.as_str() {
							"sync:state_change" => {
								info!("Handling state change event for sync broadcast");

								if let Err(e) = Self::handle_state_change_event_static(
									library_id,
									data,
									&network,
									&state,
									&buffer,
									&retry_queue,
									&db,
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
									&db,
								)
								.await
								{
									warn!(error = %e, "Failed to handle shared change event");
								}
							}
							_ => {
								// Ignore other custom events
								// Note: Batch events removed - peers discover bulk changes via backfill
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

								// Trigger watermark exchange for reconnection sync
								if let Err(e) = Self::trigger_watermark_exchange(
									library_id, device_id, peer_id, &db, &network,
								)
								.await
								{
									warn!(
										peer_id = %peer_id,
										error = %e,
										"Failed to trigger watermark exchange"
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
	async fn trigger_watermark_exchange(
		library_id: Uuid,
		device_id: Uuid,
		peer_id: Uuid,
		db: &DatabaseConnection,
		network: &Arc<dyn NetworkTransport>,
	) -> Result<()> {
		info!(
			peer = %peer_id,
			device = %device_id,
			"Triggering watermark exchange with peer"
		);

		// Query our watermarks from devices table
		let (my_state_watermark, my_shared_watermark) =
			Self::query_device_watermarks(device_id, db).await;

		debug!(
			peer = %peer_id,
			my_state_watermark = ?my_state_watermark,
			my_shared_watermark = ?my_shared_watermark,
			"Sending watermark exchange request"
		);

		// Send request to peer
		let request = SyncMessage::WatermarkExchangeRequest {
			library_id,
			device_id,
			my_state_watermark,
			my_shared_watermark,
		};

		network
			.send_sync_message(peer_id, request)
			.await
			.map_err(|e| anyhow::anyhow!("Failed to send watermark exchange request: {}", e))?;

		info!(
			peer = %peer_id,
			"Watermark exchange request sent, waiting for response"
		);

		Ok(())
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
		let connected_partners = network.get_connected_sync_partners(library_id, db).await.map_err(|e| {
			warn!(error = %e, "Failed to get connected partners");
			e
		})?;

		debug!(
			count = connected_partners.len(),
			partners = ?connected_partners,
			"[Static Handler] Got connected sync partners from transport"
		);

		if connected_partners.is_empty() {
			debug!("[Static Handler] No connected sync partners to broadcast to, queuing for retry");

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

		let send_futures: Vec<_> = connected_partners
			.iter()
			.map(|&partner| {
				let network = network.clone();
				let msg = message.clone();
				async move {
					match tokio::time::timeout(
						std::time::Duration::from_secs(SYNC_MESSAGE_TIMEOUT_SECS),
						network.send_sync_message(partner, msg),
					)
					.await
					{
						Ok(Ok(())) => (partner, Ok(())),
						Ok(Err(e)) => (partner, Err(e)),
						Err(_) => (
							partner,
							Err(anyhow::anyhow!(
								"Send timeout after {}s",
								SYNC_MESSAGE_TIMEOUT_SECS
							)),
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
		let connected_partners = network.get_connected_sync_partners(library_id, db).await.map_err(|e| {
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

		let send_futures: Vec<_> = connected_partners
			.iter()
			.map(|&partner| {
				let network = network.clone();
				let msg = message.clone();
				async move {
					match tokio::time::timeout(
						std::time::Duration::from_secs(SYNC_MESSAGE_TIMEOUT_SECS),
						network.send_sync_message(partner, msg),
					)
					.await
					{
						Ok(Ok(())) => (partner, Ok(())),
						Ok(Err(e)) => (partner, Err(e)),
						Err(_) => (
							partner,
							Err(anyhow::anyhow!(
								"Send timeout after {}s",
								SYNC_MESSAGE_TIMEOUT_SECS
							)),
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

		// Update state watermark for the device
		self.update_state_watermark(change.device_id, change.timestamp).await?;

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
			.map_err(|e| anyhow::anyhow!("Failed to apply shared change: {}", e))?;

		// Record this change in our peer log (track what we've applied)
		self.peer_log
			.append(entry.clone())
			.await
			.map_err(|e| anyhow::anyhow!("Failed to append to peer log: {}", e))?;

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
	///
	/// This is completely domain-agnostic - it delegates to the Syncable trait
	/// implementations in each entity. No switch statements, no domain logic.
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
