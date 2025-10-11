//! Backfill logic for new devices joining a library
//!
//! Handles the complete backfill flow:
//! 1. Peer selection
//! 2. Device-owned state sync
//! 3. Shared resource sync
//! 4. Buffer processing
//! 5. Transition to ready

use super::{
	peer::PeerSync,
	protocol_handler::{LogSyncHandler, StateSyncHandler},
	state::{select_backfill_peer, BackfillCheckpoint, DeviceSyncState, PeerInfo},
};
use crate::{
	infra::sync::{SharedChangeEntry, HLC},
	service::network::protocol::sync::messages::{StateRecord, SyncMessage},
};
use anyhow::Result;
use chrono::{DateTime, Utc};
use std::sync::Arc;
use tracing::{info, warn};
use uuid::Uuid;

/// Manages backfill process for new devices
pub struct BackfillManager {
	library_id: Uuid,
	device_id: Uuid,
	peer_sync: Arc<PeerSync>,
	state_handler: Arc<StateSyncHandler>,
	log_handler: Arc<LogSyncHandler>,
}

impl BackfillManager {
	pub fn new(
		library_id: Uuid,
		device_id: Uuid,
		peer_sync: Arc<PeerSync>,
		state_handler: Arc<StateSyncHandler>,
		log_handler: Arc<LogSyncHandler>,
	) -> Self {
		Self {
			library_id,
			device_id,
			peer_sync,
			state_handler,
			log_handler,
		}
	}

	/// Start complete backfill process
	pub async fn start_backfill(&self, available_peers: Vec<PeerInfo>) -> Result<()> {
		info!(
			library_id = %self.library_id,
			device_id = %self.device_id,
			peer_count = available_peers.len(),
			"Starting backfill process"
		);

		// Phase 1: Select best peer
		let selected_peer =
			select_backfill_peer(available_peers).map_err(|e| anyhow::anyhow!("{}", e))?;

		info!(
			selected_peer = %selected_peer,
			"Selected backfill peer"
		);

		// Set state to Backfilling
		{
			let mut state = self.peer_sync.state.write().await;
			*state = DeviceSyncState::Backfilling {
				peer: selected_peer,
				progress: 0,
			};
		}

		// Phase 2: Backfill device-owned state
		self.backfill_device_owned_state(selected_peer).await?;

		// Phase 3: Backfill shared resources
		self.backfill_shared_resources(selected_peer).await?;

		// Phase 4: Transition to ready (processes buffer)
		self.peer_sync.transition_to_ready().await?;

		info!("Backfill complete, device is ready");

		Ok(())
	}

	/// Backfill device-owned state from all peers in dependency order
	async fn backfill_device_owned_state(&self, primary_peer: Uuid) -> Result<()> {
		info!("Backfilling device-owned state");

		// Compute sync order based on model dependencies to prevent FK violations
		let sync_order = crate::infra::sync::compute_registry_sync_order()
			.await
			.map_err(|e| anyhow::anyhow!("Failed to compute sync order: {}", e))?;

		info!(
			sync_order = ?sync_order,
			"Computed dependency-ordered sync sequence"
		);

		// Filter to only device-owned models
		let mut model_types = Vec::new();
		for model in sync_order {
			if crate::infra::sync::is_device_owned(&model).await {
				model_types.push(model);
			}
		}

		// TODO: Get list of all peers, not just primary
		// For now, just backfill from primary peer
		let checkpoint = self
			.backfill_peer_state(primary_peer, model_types.clone(), None)
			.await?;

		info!(
			progress = checkpoint.progress,
			"Device-owned state backfill complete"
		);

		Ok(())
	}

	/// Backfill state from a specific peer
	async fn backfill_peer_state(
		&self,
		peer: Uuid,
		model_types: Vec<String>,
		checkpoint: Option<BackfillCheckpoint>,
	) -> Result<BackfillCheckpoint> {
		let mut current_checkpoint = checkpoint.unwrap_or_else(|| BackfillCheckpoint::start(peer));

		for model_type in model_types {
			if current_checkpoint.completed_models.contains(&model_type) {
				continue; // Already done
			}

			info!(
				peer = %peer,
				model_type = %model_type,
				"Backfilling model type"
			);

			// Request state in batches
			loop {
				let response = self
					.request_state_batch(peer, vec![model_type.clone()], None, None, 10_000)
					.await?;

				// Apply batch
				if let SyncMessage::StateResponse {
					records,
					has_more,
					checkpoint: chk,
					..
				} = response
				{
					for record in records {
						// Apply via registry
						let db = self.peer_sync.db().clone();
						crate::infra::sync::registry::apply_state_change(
							&model_type,
							record.data,
							db,
						)
						.await
						.map_err(|e| anyhow::anyhow!("{}", e))?;
					}

					current_checkpoint.update(chk, 0.5); // TODO: Calculate actual progress
					current_checkpoint.save().await?;

					if !has_more {
						break;
					}
				}
			}

			current_checkpoint.mark_completed(model_type);
		}

		Ok(current_checkpoint)
	}

	/// Backfill shared resources
	async fn backfill_shared_resources(&self, peer: Uuid) -> Result<()> {
		info!("Backfilling shared resources");

		// Request shared changes from peer
		let response = self.request_shared_changes(peer, None, 10_000).await?;

		if let SyncMessage::SharedChangeResponse {
			entries,
			current_state,
			..
		} = response
		{
			// Apply entries in HLC order (already sorted from peer)
			for entry in entries {
				self.log_handler.handle_shared_change(entry).await?;
			}

			// If logs were pruned, use current_state fallback
			if let Some(state) = current_state {
				info!("Applying current shared state (logs were pruned)");
				// TODO: Deserialize and insert tags, albums, etc.
			}
		}

		info!("Shared resources backfill complete");

		Ok(())
	}

	/// Request state batch from peer (stub - needs network integration)
	async fn request_state_batch(
		&self,
		peer: Uuid,
		model_types: Vec<String>,
		device_id: Option<Uuid>,
		since: Option<DateTime<Utc>>,
		batch_size: usize,
	) -> Result<SyncMessage> {
		// TODO: Send StateRequest via network
		// For now, return empty response
		Ok(SyncMessage::StateResponse {
			library_id: self.library_id,
			model_type: "location".to_string(),
			device_id: peer,
			records: Vec::new(),
			checkpoint: None,
			has_more: false,
		})
	}

	/// Request shared changes from peer (stub - needs network integration)
	async fn request_shared_changes(
		&self,
		peer: Uuid,
		since_hlc: Option<HLC>,
		limit: usize,
	) -> Result<SyncMessage> {
		// TODO: Send SharedChangeRequest via network
		// For now, return empty response
		Ok(SyncMessage::SharedChangeResponse {
			library_id: self.library_id,
			entries: Vec::new(),
			current_state: None,
			has_more: false,
		})
	}

	/// Handle peer disconnection during backfill
	pub async fn on_peer_disconnected(&self, peer_id: Uuid) -> Result<()> {
		let state = self.peer_sync.state().await;

		if let DeviceSyncState::Backfilling { peer, .. } = state {
			if peer == peer_id {
				warn!(
					peer_id = %peer_id,
					"Backfill peer disconnected, need to switch"
				);

				// TODO: Save checkpoint, select new peer, resume
				// For now, just log
			}
		}

		Ok(())
	}
}
