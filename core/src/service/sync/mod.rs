//! Sync Service - Real-time library synchronization (Leaderless)
//!
//! Background service that handles real-time peer-to-peer sync using hybrid model:
//! - State-based sync for device-owned data
//! - Log-based sync with HLC for shared resources

pub mod backfill;
pub mod metrics;
pub mod peer;
pub mod protocol_handler;
pub mod retry_queue;
pub mod state;

// No longer need SyncLogDb in leaderless architecture
use crate::library::Library;
use crate::service::network::protocol::SyncProtocolHandler;
use anyhow::Result;
use async_trait::async_trait;
use once_cell::sync::OnceCell;
pub use peer::PeerSync;
pub use state::{
	select_backfill_peer, BackfillCheckpoint, BufferQueue, BufferedUpdate, DeviceSyncState,
	PeerInfo, StateChangeMessage,
};
use crate::infra::db::entities;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, info, warn};
use uuid::Uuid;

pub use metrics::SyncMetricsCollector;

pub use backfill::BackfillManager;
pub use protocol_handler::{LogSyncHandler, StateSyncHandler};

/// Sync service for a library (Leaderless)
///
/// This service runs in the background for the lifetime of an open library,
/// handling real-time peer-to-peer synchronization.
pub struct SyncService {
	/// Sync configuration
	config: Arc<crate::infra::sync::SyncConfig>,

	/// Peer sync handler
	peer_sync: Arc<PeerSync>,

	/// Backfill manager for orchestrating initial sync
	backfill_manager: Arc<BackfillManager>,

	/// Metrics collector for observability
	metrics: Arc<SyncMetricsCollector>,

	/// Whether the service is running
	is_running: Arc<AtomicBool>,

	/// Shutdown signal
	shutdown_tx: Arc<Mutex<Option<tokio::sync::broadcast::Sender<()>>>>,
}

impl SyncService {
	/// Create a new sync service from a Library reference
	///
	/// Note: Called via `Library::init_sync_service()`, not directly.
	pub async fn new_from_library(
		library: &Library,
		device_id: Uuid,
		network: Arc<dyn crate::infra::sync::NetworkTransport>,
	) -> Result<Self> {
		Self::new_from_library_with_config(
			library,
			device_id,
			network,
			crate::infra::sync::SyncConfig::default(),
		)
		.await
	}

	/// Create a new sync service with custom configuration
	pub async fn new_from_library_with_config(
		library: &Library,
		device_id: Uuid,
		network: Arc<dyn crate::infra::sync::NetworkTransport>,
		config: crate::infra::sync::SyncConfig,
	) -> Result<Self> {
		let config = Arc::new(config);
		let library_id = library.id();

		// Create sync.db (peer log) for this device
		let peer_log = Arc::new(
			crate::infra::sync::PeerLog::open(library_id, device_id, library.path())
				.await
				.map_err(|e| anyhow::anyhow!("Failed to open sync.db: {}", e))?,
		);

		// Create metrics collector
		let metrics = Arc::new(SyncMetricsCollector::new());

		// Create peer sync handler with network transport
		let peer_sync = Arc::new(PeerSync::new(library, device_id, peer_log, network, config.clone(), metrics.clone()).await?);

		// Create protocol handlers
		let state_handler = Arc::new(StateSyncHandler::new(library_id, library.db().clone()));
		let log_handler = Arc::new(LogSyncHandler::new(
			library_id,
			library.db().clone(),
			peer_sync.clone(),
		));

		// Create backfill manager for automatic orchestration
		let backfill_manager = Arc::new(BackfillManager::new(
			library_id,
			device_id,
			peer_sync.clone(),
			state_handler,
			log_handler,
			config.clone(),
			metrics.clone(),
		));

		// Set backfill manager reference on peer_sync (for triggering catch-up)
		peer_sync.set_backfill_manager(Arc::downgrade(&backfill_manager)).await;

		info!(
			library_id = %library_id,
			device_id = %device_id,
			batch_size = config.batching.backfill_batch_size,
			retention_days = config.retention.tombstone_max_retention_days,
			"Created peer sync service with config"
		);

		Ok(Self {
			config,
			peer_sync,
			backfill_manager,
			metrics,
			is_running: Arc::new(AtomicBool::new(false)),
			shutdown_tx: Arc::new(Mutex::new(None)),
		})
	}

	/// Get the current sync configuration
	pub fn config(&self) -> &Arc<crate::infra::sync::SyncConfig> {
		&self.config
	}

	/// Get the peer sync handler
	pub fn peer_sync(&self) -> &Arc<PeerSync> {
		&self.peer_sync
	}

	/// Get the backfill manager
	pub fn backfill_manager(&self) -> &Arc<BackfillManager> {
		&self.backfill_manager
	}

	/// Get the metrics collector
	pub fn metrics(&self) -> &Arc<SyncMetricsCollector> {
		&self.metrics
	}

	/// Emit metrics update event
	pub async fn emit_metrics_event(&self, library_id: Uuid) {
		// Create a snapshot of current metrics
		let snapshot = crate::service::sync::metrics::snapshot::SyncMetricsSnapshot::from_metrics(self.metrics.metrics()).await;
		
		// Emit to sync event bus (non-critical, can be dropped if bus is under load)
		let metrics_data = serde_json::to_value(&snapshot).unwrap_or_else(|_| serde_json::json!({}));
		
		self.peer_sync.sync_events.emit(crate::infra::sync::SyncEvent::MetricsUpdated {
			library_id,
			metrics: metrics_data,
		});
	}

	/// Main sync loop (spawned as background task)
	///
	/// This is the orchestration layer that:
	/// - Detects when backfill is needed (Uninitialized state)
	/// - Triggers automatic backfill from available peers
	/// - Runs periodic maintenance (log pruning, heartbeats)
	async fn run_sync_loop(
		config: Arc<crate::infra::sync::SyncConfig>,
		peer_sync: Arc<PeerSync>,
		backfill_manager: Arc<BackfillManager>,
		is_running: Arc<AtomicBool>,
		mut shutdown_rx: tokio::sync::broadcast::Receiver<()>,
	) {
		info!("Starting peer sync loop (leaderless)");

		let mut backfill_attempted = false;

		tokio::select! {
			_ = async {
				loop {
					// Check current sync state
					let state = peer_sync.state().await;

					match state {
						DeviceSyncState::Uninitialized => {
							if !backfill_attempted {
								// Get available sync partners from network (library-scoped)
								match peer_sync.network().get_connected_sync_partners(
									peer_sync.library_id(),
									peer_sync.db(),
								).await {
									Ok(partners) if !partners.is_empty() => {
										info!("Device uninitialized - attempting automatic backfill");
										info!("Found {} connected partners, starting backfill", partners.len());
										backfill_attempted = true;

										// Convert to PeerInfo (TODO: get real latency metrics)
										let peer_info: Vec<PeerInfo> = partners
											.into_iter()
											.map(|device_id| PeerInfo {
												device_id,
												latency_ms: 50.0, // TODO: Measure actual latency
												is_online: true,
												has_complete_state: true, // Assume peers have full state
												active_syncs: 0,
											})
											.collect();

										// Start backfill process
										match backfill_manager.start_backfill(peer_info).await {
											Ok(()) => {
												info!("Automatic backfill completed successfully");
											}
											Err(e) => {
												warn!("Automatic backfill failed: {}", e);
												// Reset flag to retry on next loop
												backfill_attempted = false;
											}
										}
									}
									Ok(_) => {
										// No partners available - silently retry on next loop
										backfill_attempted = false;
									}
									Err(e) => {
										warn!("Failed to get connected partners: {}", e);
										backfill_attempted = false; // Retry
									}
								}
							}
						}

						DeviceSyncState::Ready => {
							// Check for connected partners and catch up if watermarks are outdated
							match peer_sync.network().get_connected_sync_partners(
								peer_sync.library_id(),
								peer_sync.db(),
							).await {
								Ok(partners) if !partners.is_empty() => {
									// Check if we need to catch up
									let our_device = match entities::device::Entity::find()
										.filter(entities::device::Column::Uuid.eq(peer_sync.device_id()))
										.one(peer_sync.db().as_ref())
										.await
									{
										Ok(Some(device)) => device,
										Ok(None) => continue,
										Err(e) => {
											debug!("Failed to query device record: {}", e);
											continue;
										}
									};

									// Trigger catch-up if we haven't synced recently
									let should_catch_up = if let Some(last_sync) = our_device.last_sync_at {
										let time_since_sync = chrono::Utc::now().signed_duration_since(last_sync);
										time_since_sync.num_seconds() > 60
									} else {
										true
									};

									if should_catch_up {
										// Get current watermarks from sync.db
										let (state_watermark, shared_watermark) = peer_sync.get_watermarks().await;
										
										info!(
											"Triggering incremental catch-up since watermarks: state={:?}, shared={:?}",
											state_watermark,
											shared_watermark
										);

										// Pick first partner for catch-up
										let catch_up_peer = partners[0];

										// Transition to CatchingUp state
										{
											let mut state = peer_sync.state.write().await;
											*state = DeviceSyncState::CatchingUp { buffered_count: 0 };
										}

										// Perform incremental catch-up using watermarks
										// Convert HLC to string for API
										let shared_watermark_str = shared_watermark.map(|hlc| hlc.to_string());
										
										match backfill_manager.catch_up_from_peer(
											catch_up_peer,
											state_watermark,
											shared_watermark_str,
										).await {
											Ok(()) => {
												info!("Incremental catch-up completed");
												// Transition back to Ready
												let mut state = peer_sync.state.write().await;
												*state = DeviceSyncState::Ready;
											}
											Err(e) => {
												warn!("Incremental catch-up failed: {}", e);
												// Transition back to Ready even on error
												let mut state = peer_sync.state.write().await;
												*state = DeviceSyncState::Ready;
											}
										}
									}
								}
								Ok(_) => {}
								Err(e) => {
									debug!("Failed to get connected partners: {}", e);
								}
							}
						}

						DeviceSyncState::Backfilling { .. } | DeviceSyncState::CatchingUp { .. } => {
							// In progress, wait
						}

						DeviceSyncState::Paused => {
							// Sync paused by user or offline, skip
						}
					}

					// Sleep before next iteration (configurable)
					tokio::time::sleep(tokio::time::Duration::from_secs(config.network.sync_loop_interval_secs))
						.await;
				}
			} => {
				info!("Peer sync loop ended");
			}
			_ = shutdown_rx.recv() => {
				info!("Peer sync loop shutdown signal received");
			}
		}

		is_running.store(false, Ordering::SeqCst);
		info!("Sync loop stopped");
	}

	/// Unified pruning task for sync coordination data
	///
	/// Prunes both peer log (shared resources) and tombstones (device-owned deletions)
	/// using the same acknowledgment-based pattern.
	async fn run_pruning_task(
		config: Arc<crate::infra::sync::SyncConfig>,
		peer_sync: Arc<PeerSync>,
	) {
		let interval_secs = config.monitoring.pruning_interval_secs;
		let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(interval_secs));

		info!("Starting unified pruning task (interval: {}s)", interval_secs);

		loop {
			interval.tick().await;

			if let Err(e) = Self::prune_sync_coordination_data(&config, &peer_sync).await {
				warn!(
					library_id = %peer_sync.library_id(),
					error = %e,
					"Failed to prune sync coordination data"
				);
			}
		}
	}

	/// Prune sync coordination data (tombstones and peer log)
	///
	/// Uses unified acknowledgment-based pruning for both:
	/// - Tombstones (device-owned deletions) - pruned when all devices synced past them
	/// - Peer log (shared resources) - pruned when all peers acknowledged
	async fn prune_sync_coordination_data(
		config: &crate::infra::sync::SyncConfig,
		peer_sync: &PeerSync,
	) -> Result<()> {
		// 1. Prune tombstones (device-owned deletions, in library.db)
		let pruned_tombstones = Self::prune_tombstones_acked(config, peer_sync.db()).await?;

		// 2. Prune peer log (shared resources, in sync.db)
		let pruned_peer_log = peer_sync.peer_log().prune_acked().await.unwrap_or(0);

		if pruned_tombstones > 0 || pruned_peer_log > 0 {
			info!(
				library_id = %peer_sync.library_id(),
				tombstones_pruned = pruned_tombstones,
				peer_log_pruned = pruned_peer_log,
				"Pruned sync coordination data (ack-based)"
			);
		}

		Ok(())
	}

	/// Prune tombstones that all devices have synced past
	///
	/// Note: With per-resource watermarks, this is now a simpler time-based pruning.
	/// Tombstones older than max retention are pruned automatically.
	async fn prune_tombstones_acked(
		config: &crate::infra::sync::SyncConfig,
		db: &Arc<sea_orm::DatabaseConnection>,
	) -> Result<usize> {
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

		// Use time-based pruning with max retention limit
		// This prevents one offline device from blocking pruning forever
		let max_retention = chrono::Utc::now()
			- chrono::Duration::days(config.retention.tombstone_max_retention_days as i64);

		// Prune tombstones older than max retention
		let result = entities::device_state_tombstone::Entity::delete_many()
			.filter(entities::device_state_tombstone::Column::DeletedAt.lt(max_retention))
			.exec(db.as_ref())
			.await?;

		if result.rows_affected > 0 {
			debug!(
				pruned = result.rows_affected,
				cutoff = %max_retention,
				"Pruned tombstones older than max retention"
			);
		}

		Ok(result.rows_affected as usize)
	}
}

#[async_trait]
impl crate::service::Service for SyncService {
	fn name(&self) -> &'static str {
		"sync_service"
	}

	fn is_running(&self) -> bool {
		self.is_running.load(Ordering::SeqCst)
	}

	async fn start(&self) -> Result<()> {
		if self.is_running.load(Ordering::SeqCst) {
			warn!("Sync service already running");
			return Ok(());
		}

		info!("Starting peer sync service (leaderless)");

		// Create shutdown channel
		let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);
		*self.shutdown_tx.lock().await = Some(shutdown_tx);

		// Mark as running
		self.is_running.store(true, Ordering::SeqCst);

		// Start peer sync
		self.peer_sync.start().await?;

		// Spawn sync loop with orchestration
		let config = self.config.clone();
		let peer_sync = self.peer_sync.clone();
		let backfill_manager = self.backfill_manager.clone();
		let is_running = self.is_running.clone();
		tokio::spawn(async move {
			Self::run_sync_loop(config, peer_sync, backfill_manager, is_running, shutdown_rx).await;
		});

		// Spawn unified pruning task (runs hourly)
		let config_clone = self.config.clone();
		let peer_sync_clone = self.peer_sync.clone();
		tokio::spawn(async move {
			Self::run_pruning_task(config_clone, peer_sync_clone).await;
		});

		// Spawn metrics persistence task (runs every 5 minutes)
		let metrics = self.metrics.clone();
		let library_id = self.peer_sync.library_id();
		let db = self.peer_sync.db().clone();
		tokio::spawn(async move {
			run_metrics_persistence_task(metrics, library_id, db).await;
		});

		info!("Peer sync service started (with pruning task)");

		Ok(())
	}

	async fn stop(&self) -> Result<()> {
		if !self.is_running.load(Ordering::SeqCst) {
			return Ok(());
		}

		info!("Stopping peer sync service");

		// Stop peer sync
		self.peer_sync.stop().await?;

		// Send shutdown signal
		if let Some(shutdown_tx) = self.shutdown_tx.lock().await.as_ref() {
			let _ = shutdown_tx.send(());
		}

		// Mark as stopped
		self.is_running.store(false, Ordering::SeqCst);

		info!("Peer sync service stopped");

		Ok(())
	}
}

/// Background task for persisting metrics snapshots
async fn run_metrics_persistence_task(
	metrics: Arc<SyncMetricsCollector>,
	library_id: Uuid,
	db: Arc<sea_orm::DatabaseConnection>,
) {
	let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(300)); // 5 minutes
	
	info!("Starting metrics persistence task (interval: 5m)");
	
	loop {
		interval.tick().await;
		
		// Create snapshot
		let snapshot = crate::service::sync::metrics::snapshot::SyncMetricsSnapshot::from_metrics(metrics.metrics()).await;
		
		// Store in database
		if let Err(e) = crate::service::sync::metrics::persistence::store_metrics_snapshot(
			&db,
			library_id,
			snapshot,
		).await {
			warn!(
				library_id = %library_id,
				error = %e,
				"Failed to persist metrics snapshot"
			);
		}
	}
}
