//! Sync Service - Real-time library synchronization (Leaderless)
//!
//! Background service that handles real-time peer-to-peer sync using hybrid model:
//! - State-based sync for device-owned data
//! - Log-based sync with HLC for shared resources

pub mod applier;
pub mod backfill;
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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tracing::{info, warn};
use uuid::Uuid;

pub use applier::SyncApplier;
pub use backfill::BackfillManager;
pub use protocol_handler::{LogSyncHandler, StateSyncHandler};

/// Sync service for a library (Leaderless)
///
/// This service runs in the background for the lifetime of an open library,
/// handling real-time peer-to-peer synchronization.
pub struct SyncService {
	/// Peer sync handler
	peer_sync: Arc<PeerSync>,

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
		let library_id = library.id();

		// Create sync.db (peer log) for this device
		let peer_log = Arc::new(
			crate::infra::sync::PeerLog::open(library_id, device_id, library.path())
				.await
				.map_err(|e| anyhow::anyhow!("Failed to open sync.db: {}", e))?,
		);

		// Create peer sync handler with network transport
		let peer_sync = Arc::new(PeerSync::new(library, device_id, peer_log, network).await?);

		info!(
			library_id = %library_id,
			device_id = %device_id,
			"Created peer sync service (leaderless)"
		);

		Ok(Self {
			peer_sync,
			is_running: Arc::new(AtomicBool::new(false)),
			shutdown_tx: Arc::new(Mutex::new(None)),
		})
	}

	/// Get the peer sync handler
	pub fn peer_sync(&self) -> &Arc<PeerSync> {
		&self.peer_sync
	}

	/// Main sync loop (spawned as background task)
	async fn run_sync_loop(
		peer_sync: Arc<PeerSync>,
		is_running: Arc<AtomicBool>,
		mut shutdown_rx: tokio::sync::broadcast::Receiver<()>,
	) {
		info!("Starting peer sync loop (leaderless)");

		// TODO: Implement periodic tasks:
		// - Process buffer queue
		// - Prune sync log
		// - Heartbeat to peers
		// - Reconnect to offline peers

		tokio::select! {
			_ = async {
				loop {
					tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
					// Periodic sync tasks
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

		// Spawn sync loop
		let peer_sync = self.peer_sync.clone();
		let is_running = self.is_running.clone();
		tokio::spawn(async move {
			Self::run_sync_loop(peer_sync, is_running, shutdown_rx).await;
		});

		info!("Peer sync service started");

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
