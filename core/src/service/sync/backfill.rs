//! Backfill logic for new devices joining a library
//!
//! Handles the complete backfill flow:
//! 1. Peer selection
//! 2. Device-owned state sync
//! 3. Shared resource sync
//! 4. Buffer processing
//! 5. Transition to ready

use super::{
	metrics::SyncMetricsCollector,
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
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{oneshot, Mutex};
use tracing::{info, warn};
use uuid::Uuid;

/// Manages backfill process for new devices
pub struct BackfillManager {
	library_id: Uuid,
	device_id: Uuid,
	peer_sync: Arc<PeerSync>,
	state_handler: Arc<StateSyncHandler>,
	log_handler: Arc<LogSyncHandler>,
	config: Arc<crate::infra::sync::SyncConfig>,
	metrics: Arc<SyncMetricsCollector>,

	/// Pending state request channel (backfill is sequential, only one at a time)
	pending_state_response: Arc<Mutex<Option<oneshot::Sender<SyncMessage>>>>,

	/// Pending shared change request channel
	pending_shared_response: Arc<Mutex<Option<oneshot::Sender<SyncMessage>>>>,
}

impl BackfillManager {
	pub fn new(
		library_id: Uuid,
		device_id: Uuid,
		peer_sync: Arc<PeerSync>,
		state_handler: Arc<StateSyncHandler>,
		log_handler: Arc<LogSyncHandler>,
		config: Arc<crate::infra::sync::SyncConfig>,
		metrics: Arc<SyncMetricsCollector>,
	) -> Self {
		Self {
			library_id,
			device_id,
			peer_sync,
			state_handler,
			log_handler,
			config,
			metrics,
			pending_state_response: Arc::new(Mutex::new(None)),
			pending_shared_response: Arc::new(Mutex::new(None)),
		}
	}

	/// Deliver a StateResponse to waiting request
	///
	/// Called by protocol handler when StateResponse is received.
	pub async fn deliver_state_response(&self, response: SyncMessage) -> Result<()> {
		let mut pending = self.pending_state_response.lock().await;

		if let Some(sender) = pending.take() {
			sender.send(response).map_err(|_| {
				anyhow::anyhow!("Failed to deliver StateResponse - receiver dropped")
			})?;
		} else {
			warn!("Received StateResponse but no pending request");
		}

		Ok(())
	}

	/// Deliver a SharedChangeResponse to waiting request
	///
	/// Called by protocol handler when SharedChangeResponse is received.
	pub async fn deliver_shared_response(&self, response: SyncMessage) -> Result<()> {
		let mut pending = self.pending_shared_response.lock().await;

		if let Some(sender) = pending.take() {
			sender.send(response).map_err(|_| {
				anyhow::anyhow!("Failed to deliver SharedChangeResponse - receiver dropped")
			})?;
		} else {
			warn!("Received SharedChangeResponse but no pending request");
		}

		Ok(())
	}

	/// Start complete backfill process
	pub async fn start_backfill(&self, available_peers: Vec<PeerInfo>) -> Result<()> {
		// Record metrics
		self.metrics.record_backfill_session_start();

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

		// Phase 2: Backfill shared resources FIRST (entries depend on content_identities)
		let max_shared_hlc = self.backfill_shared_resources(selected_peer).await?;

		// Phase 3: Backfill device-owned state (after shared dependencies exist)
		// For initial backfill, don't use watermark (get everything)
		let final_state_checkpoint = self.backfill_device_owned_state(selected_peer, None).await?;

		// Phase 3.5: Rebuild closure tables (safety measure)
		// The per-entry rebuild in apply_state_change() should have handled entry_closure,
		// but run bulk rebuilds as safety measure in case of any missed entries or out-of-order syncing
		info!("Rebuilding closure tables after backfill as safety measure...");
		let db = self.peer_sync.db();

		// Rebuild entry_closure
		if let Err(e) = crate::infra::db::entities::entry::Model::rebuild_all_entry_closures(db).await {
			tracing::warn!("Failed to rebuild entry_closure table: {}", e);
			// Don't fail backfill, just warn
		}

		// Rebuild tag_closure from tag_relationships
		// Note: tag_closure is derived from tag_relationship records, so we need to rebuild
		// it after all tag_relationships have been synced
		if let Err(e) = rebuild_tag_closure_table(db).await {
			tracing::warn!("Failed to rebuild tag_closure table: {}", e);
			// Don't fail backfill, just warn
		}

		// Phase 4: Transition to ready (processes buffer)
		self.peer_sync.transition_to_ready().await?;

		// Phase 5: Set initial watermarks from actual received data (not local DB query)
		self.set_initial_watermarks_after_backfill(final_state_checkpoint, max_shared_hlc).await?;

		// Record metrics
		self.metrics.record_backfill_session_complete();

		info!("Backfill complete, device is ready");

		Ok(())
	}

	/// Perform incremental catch-up using watermarks
	///
	/// Called when device is Ready and reconnects after offline period.
	/// Only fetches changes newer than our watermarks.
	pub async fn catch_up_from_peer(
		&self,
		peer: Uuid,
		state_watermark: Option<chrono::DateTime<chrono::Utc>>,
		shared_watermark: Option<String>,
	) -> Result<()> {
		// Check watermark age - force full sync if too old (tombstones may be pruned)
		let watermark_age = state_watermark
			.map(|w| chrono::Utc::now() - w)
			.unwrap_or(chrono::Duration::max_value());

		let threshold_days = self.config.retention.force_full_sync_threshold_days;
		let effective_state_watermark = if watermark_age > chrono::Duration::days(threshold_days as i64) {
			warn!(
				"State watermark is {} days old (> {} days), forcing full sync to ensure consistency",
				watermark_age.num_days(),
				threshold_days
			);
			None // Force full sync
		} else {
			state_watermark
		};

		info!(
			peer = %peer,
			state_since = ?effective_state_watermark,
			shared_since = ?shared_watermark,
			"Starting incremental catch-up"
		);

		// Backfill shared resources FIRST (device-owned models depend on them)
		// For now, just do full backfill of shared resources
		// TODO: Parse HLC from string watermark when HLC implements FromStr
		let max_shared_hlc = self.backfill_shared_resources(peer).await?;

		// Backfill device-owned state since watermark (after shared dependencies exist)
		let final_state_checkpoint = self.backfill_device_owned_state(peer, effective_state_watermark).await?;

		// Update watermarks from actual received data (not local DB query)
		self.set_initial_watermarks_after_backfill(final_state_checkpoint, max_shared_hlc).await?;

		// Record metrics
		self.metrics.record_backfill_session_complete();

		info!("Incremental catch-up complete");
		Ok(())
	}

	/// Backfill device-owned state from all peers in dependency order
	///
	/// Uses per-resource watermarks for each model type to enable independent sync progress.
	/// Returns the final checkpoint string (timestamp|uuid) for global watermark update (legacy).
	async fn backfill_device_owned_state(
		&self,
		primary_peer: Uuid,
		_since_watermark: Option<chrono::DateTime<chrono::Utc>>,  // Deprecated: use per-resource watermarks
	) -> Result<Option<String>> {
		info!("Backfilling device-owned state with per-resource watermarks");

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

		// Backfill each resource type with its own watermark
		let mut final_checkpoint: Option<String> = None;
		for model_type in model_types {
			// Get per-resource watermark for this model type
			let resource_watermark = self.peer_sync
				.get_resource_watermark(primary_peer, &model_type)
				.await?;

			info!(
				model_type = %model_type,
				watermark = ?resource_watermark,
				"Backfilling resource type with per-resource watermark"
			);

			// Backfill this resource type
			let checkpoint = self
				.backfill_peer_state(
					primary_peer,
					vec![model_type.clone()],
					None,
					resource_watermark,  // Per-resource watermark!
				)
				.await?;

			info!(
				model_type = %model_type,
				progress = checkpoint.progress,
				final_checkpoint = ?checkpoint.resume_token,
				"Resource type backfill complete"
			);

			// Keep the last checkpoint for legacy compatibility
			final_checkpoint = checkpoint.resume_token;
		}

		info!("Device-owned state backfill complete (all resource types)");

		// Return the final checkpoint for legacy watermark update
		Ok(final_checkpoint)
	}

	/// Backfill state from a specific peer
	///
	/// If `since_watermark` is provided, only fetches changes newer than the watermark.
	async fn backfill_peer_state(
		&self,
		peer: Uuid,
		model_types: Vec<String>,
		checkpoint: Option<BackfillCheckpoint>,
		since_watermark: Option<chrono::DateTime<chrono::Utc>>,
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

			// Request state in batches with cursor-based pagination
			let mut cursor_checkpoint: Option<String> = None;
			loop {
				let response = self
					.request_state_batch(
						peer,
						vec![model_type.clone()],
						cursor_checkpoint.clone(),
						since_watermark,
						self.config.batching.backfill_batch_size,
					)
					.await?;

				// Apply batch
				if let SyncMessage::StateResponse {
					records,
					deleted_uuids,
					has_more,
					checkpoint: chk,
					..
				} = response
				{
					let db = self.peer_sync.db().clone();

					// Record data volume metrics before consuming records
					let records_count = records.len() as u64;

					// Apply updates via registry
					for record in records {
						crate::infra::sync::registry::apply_state_change(
							&model_type,
							record.data,
							db.clone(),
						)
						.await
						.map_err(|e| anyhow::anyhow!("{}", e))?;
					}

					// Record data volume metrics
					self.metrics.record_entries_synced(&model_type, records_count).await;

					// Apply deletions via registry
					for uuid in deleted_uuids {
						crate::infra::sync::registry::apply_deletion(&model_type, uuid, db.clone())
							.await
							.map_err(|e| anyhow::anyhow!("{}", e))?;
					}

					current_checkpoint.update(chk.clone(), 0.5); // TODO: Calculate actual progress
					current_checkpoint.save().await?;

					// Record pagination round
					self.metrics.record_backfill_pagination_round();

					// Update cursor for next iteration
					cursor_checkpoint = chk;

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
	async fn backfill_shared_resources(&self, peer: Uuid) -> Result<Option<crate::infra::sync::HLC>> {
		self.backfill_shared_resources_since(peer, None).await
	}

	/// Backfill shared resources since a specific HLC watermark
	/// Returns the maximum HLC received (for watermark update)
	async fn backfill_shared_resources_since(
		&self,
		peer: Uuid,
		since_hlc: Option<crate::infra::sync::HLC>,
	) -> Result<Option<crate::infra::sync::HLC>> {
		if let Some(hlc) = since_hlc {
			info!("Backfilling shared resources incrementally since {:?}", hlc);
		} else {
			info!("Backfilling shared resources (full)");
		}

		// Request shared changes from peer in batches (can be 100k+ records)
		let mut last_hlc = since_hlc;
		let mut total_applied = 0;

		loop {
			let response = self
				.request_shared_changes(peer, last_hlc, self.config.batching.backfill_batch_size)
				.await?;

			if let SyncMessage::SharedChangeResponse {
				entries,
				current_state,
				has_more,
				..
			} = response
			{
				let batch_size = entries.len();

				// Apply entries in HLC order (already sorted from peer)
				for entry in &entries {
					self.log_handler.handle_shared_change(entry.clone()).await?;
				}

				total_applied += batch_size;
				
				// Record metrics
				self.metrics.record_backfill_pagination_round();
				self.metrics.record_entries_synced("shared", batch_size as u64).await;
				
				info!("Applied {} shared changes (total: {})", batch_size, total_applied);

				// Update cursor to last HLC for next batch
				if let Some(last_entry) = entries.last() {
					last_hlc = Some(last_entry.hlc);
				}

				// Apply current_state snapshot (contains pre-sync data not in peer_log)
				if let Some(state) = current_state {
					if let Some(state_map) = state.as_object() {
						for (model_type, records_value) in state_map {
							if let Some(records_array) = records_value.as_array() {
								info!(
									model_type = %model_type,
									count = records_array.len(),
									"Applying current state snapshot for pre-sync data"
								);

								for record_value in records_array {
									if let Some(record_obj) = record_value.as_object() {
										if let (Some(uuid_value), Some(data)) =
											(record_obj.get("uuid"), record_obj.get("data"))
										{
											if let Some(uuid_str) = uuid_value.as_str() {
												if let Ok(record_uuid) = Uuid::parse_str(uuid_str) {
													// Construct a synthetic SharedChangeEntry for application
													// Generate HLC for ordering (pre-sync data gets current HLC)
													let hlc = {
														let mut hlc_gen = self.peer_sync.hlc_generator().lock().await;
														hlc_gen.next()
													};

													let entry = crate::infra::sync::SharedChangeEntry {
														hlc,
														model_type: model_type.clone(),
														record_uuid,
														change_type: crate::infra::sync::ChangeType::Insert,
														data: data.clone(),
													};

													let db = self.peer_sync.db().clone();
													if let Err(e) = crate::infra::sync::registry::apply_shared_change(entry, db).await {
														warn!(
															model_type = %model_type,
															uuid = %record_uuid,
															error = %e,
															"Failed to apply current state record"
														);
													}
												}
											}
										}
									}
								}
							}
						}
					}
				}

				// Continue if there are more entries
				if !has_more || batch_size == 0 {
					break;
				}
			} else {
				break;
			}
		}

		info!("Shared resources backfill complete (total: {} entries)", total_applied);

		// Return the max HLC from received data for accurate watermark tracking
		Ok(last_hlc)
	}

	/// Request state batch from peer
	///
	/// Sends a StateRequest via bidirectional stream and waits for StateResponse.
	async fn request_state_batch(
		&self,
		peer: Uuid,
		model_types: Vec<String>,
		checkpoint: Option<String>,
		since: Option<DateTime<Utc>>,
		batch_size: usize,
	) -> Result<SyncMessage> {
		// Create and send request
		let request = SyncMessage::StateRequest {
			library_id: self.library_id,
			model_types: model_types.clone(),
			device_id: None,
			since,
			checkpoint,
			batch_size,
		};

		// Use send_sync_request which handles bidirectional stream and response
		let response = self
			.peer_sync
			.network()
			.send_sync_request(peer, request)
			.await?;

		Ok(response)
	}

	/// Request shared changes from peer
	///
	/// Sends a SharedChangeRequest via bidirectional stream and waits for SharedChangeResponse.
	async fn request_shared_changes(
		&self,
		peer: Uuid,
		since_hlc: Option<HLC>,
		limit: usize,
	) -> Result<SyncMessage> {
		// Create and send request
		let request = SyncMessage::SharedChangeRequest {
			library_id: self.library_id,
			since_hlc,
			limit,
		};

		// Use send_sync_request which handles bidirectional stream and response
		let response = self
			.peer_sync
			.network()
			.send_sync_request(peer, request)
			.await?;

		Ok(response)
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

	/// Set initial watermarks after backfill completes
	/// Uses actual checkpoints from received data, not local database queries
	async fn set_initial_watermarks_after_backfill(
		&self,
		final_state_checkpoint: Option<String>,
		max_shared_hlc: Option<crate::infra::sync::HLC>,
	) -> Result<()> {
		self.peer_sync.set_initial_watermarks(final_state_checkpoint, max_shared_hlc).await
	}
}

/// Rebuild tag_closure table from tag_relationship records
///
/// Tag closure is derived from tag_relationship records. After syncing tag_relationships,
/// we need to rebuild the closure table to enable hierarchical tag queries.
async fn rebuild_tag_closure_table(db: &sea_orm::DatabaseConnection) -> Result<()> {
	use crate::infra::db::entities::{tag_closure, tag_relationship};
	use sea_orm::{ConnectionTrait, DbBackend, EntityTrait, PaginatorTrait, Set, Statement};

	tracing::info!("Starting tag_closure rebuild from tag_relationships...");

	// Clear existing tag_closure table
	tag_closure::Entity::delete_many()
		.exec(db)
		.await?;

	// 1. Insert self-references for all tags (depth 0)
	db.execute(Statement::from_sql_and_values(
		DbBackend::Sqlite,
		r#"
		INSERT INTO tag_closure (ancestor_id, descendant_id, depth, path_strength)
		SELECT id, id, 0, 1.0 FROM tag
		"#,
		vec![],
	))
	.await?;

	// 2. Insert direct relationships from tag_relationship (depth 1)
	db.execute(Statement::from_sql_and_values(
		DbBackend::Sqlite,
		r#"
		INSERT OR IGNORE INTO tag_closure (ancestor_id, descendant_id, depth, path_strength)
		SELECT parent_tag_id, child_tag_id, 1, strength
		FROM tag_relationship
		"#,
		vec![],
	))
	.await?;

	// 3. Recursively build transitive relationships
	let mut iteration = 0;
	loop {
		let result = db
			.execute(Statement::from_sql_and_values(
				DbBackend::Sqlite,
				r#"
				INSERT OR IGNORE INTO tag_closure (ancestor_id, descendant_id, depth, path_strength)
				SELECT tc1.ancestor_id, tc2.descendant_id, tc1.depth + tc2.depth, tc1.path_strength * tc2.path_strength
				FROM tag_closure tc1
				INNER JOIN tag_closure tc2 ON tc1.descendant_id = tc2.ancestor_id
				WHERE tc1.depth > 0 OR tc2.depth > 0
				  AND NOT EXISTS (
					SELECT 1 FROM tag_closure
					WHERE ancestor_id = tc1.ancestor_id
					  AND descendant_id = tc2.descendant_id
				  )
				"#,
				vec![],
			))
			.await?;

		iteration += 1;
		let rows_affected = result.rows_affected();

		tracing::debug!(
			iteration = iteration,
			rows_inserted = rows_affected,
			"tag_closure rebuild iteration"
		);

		if rows_affected == 0 {
			break; // No more relationships to add
		}

		if iteration > 100 {
			return Err(anyhow::anyhow!(
				"tag_closure rebuild exceeded max iterations - possible cycle"
			));
		}
	}

	let total = tag_closure::Entity::find().count(db).await?;

	tracing::info!(
		iterations = iteration,
		total_relationships = total,
		"tag_closure rebuild complete"
	);

	Ok(())
}
