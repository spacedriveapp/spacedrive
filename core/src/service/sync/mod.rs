//! Sync Service - Real-time library synchronization
//!
//! Background service that handles real-time sync between leader and follower devices.
//! - Leader: Listens for commit events, pushes NewEntries to followers
//! - Follower: Listens for NewEntries, applies changes locally

pub mod applier;
pub mod follower;
pub mod leader;

use crate::infra::sync::{SyncLogDb, SyncRole};
use crate::library::Library;
use crate::service::network::protocol::SyncProtocolHandler;
use anyhow::Result;
use async_trait::async_trait;
use once_cell::sync::OnceCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tracing::{info, warn};
use uuid::Uuid;

pub use applier::SyncApplier;
pub use follower::FollowerSync;
pub use leader::LeaderSync;

/// Sync service for a library
///
/// This service runs in the background for the lifetime of an open library,
/// handling real-time synchronization with paired devices.
pub struct SyncService {
	/// Library ID
	library_id: Uuid,

	/// Sync log database
	sync_log_db: Arc<SyncLogDb>,

	/// Event bus
	event_bus: Arc<crate::infra::event::EventBus>,

	/// Database connection
	db: Arc<crate::infra::db::Database>,

	/// Current sync role (Leader or Follower)
	role: Arc<Mutex<SyncRole>>,

	/// Whether the service is running
	is_running: Arc<AtomicBool>,

	/// Shutdown signal
	shutdown_tx: Arc<Mutex<Option<tokio::sync::broadcast::Sender<()>>>>,

	/// Leader-specific sync handler
	leader_sync: Option<Arc<LeaderSync>>,

	/// Follower-specific sync handler
	follower_sync: Option<Arc<FollowerSync>>,
}

impl SyncService {
	/// Create a new sync service from a Library reference
	///
	/// Note: Called via `Library::init_sync_service()`, not directly.
	pub async fn new_from_library(library: &Library) -> Result<Self> {
		let library_id = library.id();
		let role = {
			let leadership = library.leadership_manager().lock().await;
			leadership.get_role(library_id)
		};

		info!(
			library_id = %library_id,
			role = ?role,
			"Creating sync service"
		);

		Ok(Self {
			library_id,
			sync_log_db: library.sync_log_db().clone(),
			event_bus: library.event_bus().clone(),
			db: library.db().clone(),
			role: Arc::new(Mutex::new(role)),
			is_running: Arc::new(AtomicBool::new(false)),
			shutdown_tx: Arc::new(Mutex::new(None)),
			leader_sync: None,
			follower_sync: None,
		})
	}

	/// Get the current sync role
	pub async fn role(&self) -> SyncRole {
		*self.role.lock().await
	}

	/// Transition to a new role (called when leadership changes)
	pub async fn transition_role(&mut self, new_role: SyncRole) -> Result<()> {
		info!(
			library_id = %self.library_id,
			old_role = ?self.role().await,
			new_role = ?new_role,
			"Transitioning sync role"
		);

		// Update role
		*self.role.lock().await = new_role;

		// Restart the service with new role
		if self.is_running.load(Ordering::SeqCst) {
			use crate::service::Service;
			self.stop().await?;
			self.start().await?;
		}

		Ok(())
	}

	/// Main sync loop (spawned as background task)
	async fn run_sync_loop(
		library_id: Uuid,
		sync_log_db: Arc<SyncLogDb>,
		event_bus: Arc<crate::infra::event::EventBus>,
		db: Arc<crate::infra::db::Database>,
		role: SyncRole,
		is_running: Arc<AtomicBool>,
		mut shutdown_rx: tokio::sync::broadcast::Receiver<()>,
	) {
		info!(
			library_id = %library_id,
			role = ?role,
			"Starting sync loop"
		);

		match role {
			SyncRole::Leader => {
				// Create leader sync handler
				let leader =
					match LeaderSync::new_with_deps(library_id, sync_log_db, event_bus, db).await {
						Ok(l) => l,
						Err(e) => {
							warn!(
								library_id = %library_id,
								error = %e,
								"Failed to create leader sync handler"
							);
							return;
						}
					};

				// Run leader loop
				tokio::select! {
					_ = leader.run() => {
						info!(library_id = %library_id, "Leader sync loop ended");
					}
					_ = shutdown_rx.recv() => {
						info!(library_id = %library_id, "Leader sync loop shutdown signal received");
					}
				}
			}
			SyncRole::Follower => {
				// Create follower sync handler
				let follower = match FollowerSync::new_with_deps(library_id, sync_log_db, db).await
				{
					Ok(f) => f,
					Err(e) => {
						warn!(
							library_id = %library_id,
							error = %e,
							"Failed to create follower sync handler"
						);
						return;
					}
				};

				// Run follower loop
				tokio::select! {
					_ = follower.run() => {
						info!(library_id = %library_id, "Follower sync loop ended");
					}
					_ = shutdown_rx.recv() => {
						info!(library_id = %library_id, "Follower sync loop shutdown signal received");
					}
				}
			}
		}

		is_running.store(false, Ordering::SeqCst);
		info!(library_id = %library_id, "Sync loop stopped");
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
		let library_id = self.library_id;

		if self.is_running.load(Ordering::SeqCst) {
			warn!(library_id = %library_id, "Sync service already running");
			return Ok(());
		}

		info!(library_id = %library_id, "Starting sync service");

		// Create shutdown channel
		let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);
		*self.shutdown_tx.lock().await = Some(shutdown_tx);

		// Mark as running
		self.is_running.store(true, Ordering::SeqCst);

		// Get current role
		let role = *self.role.lock().await;

		// Spawn sync loop
		let library_id = self.library_id;
		let sync_log_db = self.sync_log_db.clone();
		let event_bus = self.event_bus.clone();
		let db = self.db.clone();
		let is_running = self.is_running.clone();
		tokio::spawn(async move {
			Self::run_sync_loop(
				library_id,
				sync_log_db,
				event_bus,
				db,
				role,
				is_running,
				shutdown_rx,
			)
			.await;
		});

		info!(
			library_id = %library_id,
			role = ?role,
			"Sync service started"
		);

		Ok(())
	}

	async fn stop(&self) -> Result<()> {
		if !self.is_running.load(Ordering::SeqCst) {
			return Ok(());
		}

		info!(library_id = %self.library_id, "Stopping sync service");

		// Send shutdown signal
		if let Some(shutdown_tx) = self.shutdown_tx.lock().await.as_ref() {
			let _ = shutdown_tx.send(());
		}

		// Mark as stopped
		self.is_running.store(false, Ordering::SeqCst);

		info!(library_id = %self.library_id, "Sync service stopped");

		Ok(())
	}
}
