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
	protocol_handler::LogSyncHandler,
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
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Manages backfill process for new devices
pub struct BackfillManager {
	library_id: Uuid,
	device_id: Uuid,
	peer_sync: Arc<PeerSync>,
	log_handler: Arc<LogSyncHandler>,
	config: Arc<crate::infra::sync::SyncConfig>,
	metrics: Arc<SyncMetricsCollector>,
	batch_aggregator: Arc<crate::infra::sync::BatchAggregator>,

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
		log_handler: Arc<LogSyncHandler>,
		config: Arc<crate::infra::sync::SyncConfig>,
		metrics: Arc<SyncMetricsCollector>,
		batch_aggregator: Arc<crate::infra::sync::BatchAggregator>,
	) -> Self {
		Self {
			library_id,
			device_id,
			peer_sync,
			log_handler,
			config,
			metrics,
			batch_aggregator,
			pending_state_response: Arc::new(Mutex::new(None)),
			pending_shared_response: Arc::new(Mutex::new(None)),
		}
	}

	/// Get metrics collector for peer latency lookups
	pub fn metrics(&self) -> &Arc<SyncMetricsCollector> {
		&self.metrics
	}

	/// Get log handler for protocol operations
	pub fn log_handler(&self) -> &Arc<LogSyncHandler> {
		&self.log_handler
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
		// Generate session ID for correlation
		let session_id = Uuid::new_v4();
		let start_time = Utc::now();

		// Record metrics
		self.metrics.record_backfill_session_start();

		// Log backfill session started
		if let Some(event_logger) = self.metrics.event_logger().read().await.as_ref() {
			use crate::infra::sync::{SyncEventLog, SyncEventType};
			use serde_json::json;

			let event = SyncEventLog::new(
				self.device_id,
				SyncEventType::BackfillSessionStarted,
				format!(
					"Backfill session started with {} available peers",
					available_peers.len()
				),
			)
			.with_correlation_id(session_id)
			.with_details(json!({
				"peer_count": available_peers.len(),
			}));

			let _ = event_logger.log(event).await;
		}

		info!(
			library_id = %self.library_id,
			device_id = %self.device_id,
			peer_count = available_peers.len(),
			session_id = %session_id,
			"Starting backfill process"
		);

		// Phase 1: Select best peer
		let selected_peer =
			select_backfill_peer(available_peers.clone()).map_err(|e| anyhow::anyhow!("{}", e))?;

		debug!(
			selected_peer = %selected_peer,
			"Selected backfill peer"
		);

		// Log peer selection details
		if let Some(event_logger) = self.metrics.event_logger().read().await.as_ref() {
			use crate::infra::sync::{SyncEventLog, SyncEventType};
			use serde_json::json;

			let peer_details: Vec<_> = available_peers
				.iter()
				.map(|p| {
					json!({
						"device_id": p.device_id.to_string(),
						"is_online": p.is_online,
						"has_complete_state": p.has_complete_state,
						"latency_ms": p.latency_ms,
						"active_syncs": p.active_syncs,
						"selected": p.device_id == selected_peer,
					})
				})
				.collect();

			let event = SyncEventLog::new(
				self.device_id,
				SyncEventType::BackfillSessionStarted,
				format!(
					"Selected peer {} from {} candidates",
					selected_peer,
					available_peers.len()
				),
			)
			.with_correlation_id(session_id)
			.with_peer(selected_peer)
			.with_details(json!({
				"candidates": peer_details,
			}));

			let _ = event_logger.log(event).await;
		}

		// Set state to Backfilling
		{
			let old_state = self.peer_sync.state().await;
			let mut state = self.peer_sync.state.write().await;
			*state = DeviceSyncState::Backfilling {
				peer: selected_peer,
				progress: 0,
			};
			info!(
				from_state = ?old_state,
				to_state = ?DeviceSyncState::Backfilling { peer: selected_peer, progress: 0 },
				peer = %selected_peer,
				"Sync state transition"
			);
		}

		// Phase 2: Backfill shared resources FIRST (entries depend on content_identities)
		let max_shared_hlc = self.backfill_shared_resources(selected_peer).await?;

		// Phase 3: Backfill device-owned state (after shared dependencies exist)
		// For initial backfill, don't use watermark (get everything)
		let final_state_checkpoint = self
			.backfill_device_owned_state(selected_peer, None)
			.await?;

		// Phase 3.5: Run post-backfill rebuilds via registry (polymorphic)
		// Models that registered post_backfill_rebuild will have their derived tables rebuilt
		// (e.g., entry_closure for entries, tag_closure for tag_relationships)
		debug!("Running post-backfill rebuilds via registry...");
		if let Err(e) =
			crate::infra::sync::registry::run_post_backfill_rebuilds(self.peer_sync.db().clone())
				.await
		{
			tracing::warn!("Post-backfill rebuild had errors: {}", e);
			// Don't fail backfill, just warn
		}

		// Phase 4: Transition to ready (processes buffer)
		self.peer_sync.transition_to_ready().await?;

		// Phase 5: Set initial watermarks from actual received data (not local DB query)
		self.set_initial_watermarks_after_backfill(
			selected_peer,
			final_state_checkpoint,
			max_shared_hlc,
		)
		.await?;

		// Record metrics
		self.metrics.record_backfill_session_complete();

		// Log backfill session completed
		if let Some(event_logger) = self.metrics.event_logger().read().await.as_ref() {
			use crate::infra::sync::{SyncEventLog, SyncEventType};

			let duration_ms = Utc::now()
				.signed_duration_since(start_time)
				.num_milliseconds()
				.max(0) as u64;

			let event = SyncEventLog::new(
				self.device_id,
				SyncEventType::BackfillSessionCompleted,
				format!("Backfill session completed successfully"),
			)
			.with_correlation_id(session_id)
			.with_peer(selected_peer)
			.with_duration_ms(duration_ms);

			let _ = event_logger.log(event).await;
		}

		info!(session_id = %session_id, "Backfill complete, device is ready");

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
		let effective_state_watermark =
			if watermark_age > chrono::Duration::days(threshold_days as i64) {
				warn!(
				"State watermark is {} days old (> {} days), forcing full sync to ensure consistency",
				watermark_age.num_days(),
				threshold_days
			);

				// Log watermark age event
				if let Some(event_logger) = self.metrics.event_logger().read().await.as_ref() {
					use crate::infra::sync::{EventSeverity, SyncEventLog, SyncEventType};
					use serde_json::json;

					let event = SyncEventLog::new(
						self.device_id,
						SyncEventType::SyncError,
						format!(
							"Watermark too old: {} days (threshold: {} days), forcing full sync",
							watermark_age.num_days(),
							threshold_days
						),
					)
					.with_peer(peer)
					.with_severity(EventSeverity::Warning)
					.with_details(json!({
						"watermark_age_days": watermark_age.num_days(),
						"threshold_days": threshold_days,
						"action": "force_full_sync",
					}));

					let _ = event_logger.log(event).await;
				}

				None // Force full sync
			} else {
				state_watermark
			};

		// Parse shared watermark HLC for incremental sync
		let since_hlc = shared_watermark
			.as_ref()
			.and_then(|s| s.parse::<crate::infra::sync::HLC>().ok());

		info!(
			peer = %peer,
			state_since = ?effective_state_watermark,
			shared_since = ?since_hlc,
			"Starting incremental catch-up"
		);

		// Backfill shared resources FIRST (device-owned models depend on them)
		// Uses parsed HLC watermark for incremental sync
		let max_shared_hlc = self
			.backfill_shared_resources_since(peer, since_hlc)
			.await?;

		// Backfill device-owned state since watermark (after shared dependencies exist)
		let final_state_checkpoint = self
			.backfill_device_owned_state(peer, effective_state_watermark)
			.await?;

		// Update watermarks from actual received data (not local DB query)
		self.set_initial_watermarks_after_backfill(peer, final_state_checkpoint, max_shared_hlc)
			.await?;

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
		_since_watermark: Option<chrono::DateTime<chrono::Utc>>, // Deprecated: use per-resource watermarks
	) -> Result<Option<String>> {
		debug!("Backfilling device-owned state with per-resource watermarks");

		// Compute sync order based on model dependencies to prevent FK violations
		let sync_order = crate::infra::sync::compute_registry_sync_order()
			.await
			.map_err(|e| anyhow::anyhow!("Failed to compute sync order: {}", e))?;

		debug!(
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
			let resource_watermark = self
				.peer_sync
				.get_resource_watermark(primary_peer, &model_type)
				.await?;

			debug!(
				model_type = %model_type,
				watermark = ?resource_watermark,
				"Backfilling resource type with per-resource watermark"
			);

			// Backfill this resource type
			let (checkpoint, max_received_timestamp) = self
				.backfill_peer_state(
					primary_peer,
					vec![model_type.clone()],
					None,
					resource_watermark, // Per-resource watermark!
				)
				.await?;

			debug!(
				model_type = %model_type,
				progress = checkpoint.progress,
				final_checkpoint = ?checkpoint.resume_token,
				"Resource type backfill complete"
			);

			// Update per-resource watermark using max timestamp from received data
			// CRITICAL: Only update if data was actually received!
			// Advancing watermark without receiving data causes permanent data loss
			if let Some(max_ts) = max_received_timestamp {
				self.peer_sync
					.update_resource_watermark(primary_peer, &model_type, max_ts)
					.await?;

				debug!(
					model_type = %model_type,
					watermark = %max_ts,
					"Updated resource watermark from received data"
				);
			} else {
				// No data received - watermark MUST NOT advance!
				// If we advanced it, we'd filter out unsynced data permanently
				debug!(
					model_type = %model_type,
					"No data received, watermark unchanged"
				);
			}

			// Keep the last checkpoint for legacy compatibility
			final_checkpoint = checkpoint.resume_token;
		}

		debug!("Device-owned state backfill complete");

		// Return the final checkpoint for legacy watermark update
		Ok(final_checkpoint)
	}

	/// Backfill state from a specific peer
	///
	/// If `since_watermark` is provided, only fetches changes newer than the watermark.
	/// Returns the checkpoint and the maximum timestamp from received data (for watermark update).
	async fn backfill_peer_state(
		&self,
		peer: Uuid,
		model_types: Vec<String>,
		checkpoint: Option<BackfillCheckpoint>,
		since_watermark: Option<chrono::DateTime<chrono::Utc>>,
	) -> Result<(BackfillCheckpoint, Option<chrono::DateTime<chrono::Utc>>)> {
		let mut current_checkpoint = checkpoint.unwrap_or_else(|| BackfillCheckpoint::start(peer));
		// Track max timestamp from ONLY received records (not initialized to watermark)
		// Initializing to since_watermark caused bug where watermark advanced even when no data received
		let mut max_timestamp: Option<chrono::DateTime<chrono::Utc>> = None;

		for model_type in model_types {
			if current_checkpoint.completed_models.contains(&model_type) {
				continue; // Already done
			}

			debug!(
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
					device_id: source_device_id,
					..
				} = response
				{
					let db = self.peer_sync.db().clone();

					// Record data volume metrics before consuming records
					let records_count = records.len() as u64;

					// Track max timestamp from received records for accurate watermark
					for record in &records {
						if let Some(max) = max_timestamp {
							if record.timestamp > max {
								max_timestamp = Some(record.timestamp);
							}
						} else {
							max_timestamp = Some(record.timestamp);
						}
					}

					// Collect all record data, sorting by parent depth for hierarchical models
					let mut record_data: Vec<serde_json::Value> =
						records.iter().map(|r| r.data.clone()).collect();

					// Sort entries so records with null parent_uuid come first
					// This ensures parents are processed before children in the same batch
					if model_type == "entry" {
						record_data.sort_by(|a, b| {
							let a_has_parent =
								a.get("parent_uuid").map(|v| !v.is_null()).unwrap_or(false);
							let b_has_parent =
								b.get("parent_uuid").map(|v| !v.is_null()).unwrap_or(false);
							a_has_parent.cmp(&b_has_parent) // false (no parent) comes before true
						});
					}

					// Get FK mappings for this model type
					let fk_mappings =
						crate::infra::sync::get_fk_mappings(&model_type).unwrap_or_default();

					// For hierarchical models (entries), process one at a time to handle parent→child ordering
					// Batch FK resolution fails because children can't find parents that haven't been inserted yet
					let processed_data = if model_type == "entry" && !fk_mappings.is_empty() {
						// Process entries individually: resolve FK → insert → resolve deps → next
						let mut succeeded = Vec::with_capacity(record_data.len());

						for data in record_data {
							// Try to resolve FKs for this single record
							let result = crate::infra::sync::batch_map_sync_json_to_local(
								vec![data.clone()],
								fk_mappings.clone(),
								&db,
							)
							.await
							.map_err(|e| anyhow::anyhow!("FK mapping failed: {}", e))?;

							if !result.succeeded.is_empty() {
								// FK resolution succeeded - add to processing list
								succeeded.extend(result.succeeded);
							} else if !result.failed.is_empty() {
								// FK resolution failed - add to dependency tracker
								for (failed_data, fk_field, missing_uuid) in result.failed {
									let record_uuid = failed_data
										.get("uuid")
										.and_then(|v| v.as_str())
										.and_then(|s| Uuid::parse_str(s).ok());

									let record_timestamp = records
										.iter()
										.find(|r| Some(r.uuid) == record_uuid)
										.map(|r| r.timestamp)
										.unwrap_or_else(chrono::Utc::now);

									if let Some(uuid) = record_uuid {
										tracing::debug!(
											model_type = %model_type,
											record_uuid = %uuid,
											fk_field = %fk_field,
											missing_uuid = %missing_uuid,
											"Entry has missing parent - adding to dependency tracker"
										);

										let state_change = super::state::StateChangeMessage {
											model_type: model_type.clone(),
											record_uuid: uuid,
											device_id: source_device_id,
											data: failed_data,
											timestamp: record_timestamp,
										};

										self.peer_sync
											.dependency_tracker()
											.add_dependency(
												missing_uuid,
												super::state::BufferedUpdate::StateChange(
													state_change,
												),
											)
											.await;
									}
								}
							}
						}

						succeeded
					} else if !fk_mappings.is_empty() && !record_data.is_empty() {
						// Non-hierarchical models: use batch FK resolution
						let result = crate::infra::sync::batch_map_sync_json_to_local(
							record_data,
							fk_mappings,
							&db,
						)
						.await
						.map_err(|e| anyhow::anyhow!("Batch FK mapping failed: {}", e))?;

						// Add failed records to dependency tracker
						if !result.failed.is_empty() {
							tracing::warn!(
								model_type = %model_type,
								failed_count = result.failed.len(),
								"Records have missing FK dependencies - adding to dependency tracker for retry"
							);

							for (failed_data, fk_field, missing_uuid) in result.failed {
								let record_uuid = failed_data
									.get("uuid")
									.and_then(|v| v.as_str())
									.and_then(|s| Uuid::parse_str(s).ok());

								let record_timestamp = records
									.iter()
									.find(|r| Some(r.uuid) == record_uuid)
									.map(|r| r.timestamp)
									.unwrap_or_else(chrono::Utc::now);

								if let Some(uuid) = record_uuid {
									tracing::debug!(
										model_type = %model_type,
										record_uuid = %uuid,
										fk_field = %fk_field,
										missing_uuid = %missing_uuid,
										"Adding record to dependency tracker"
									);

									let state_change = super::state::StateChangeMessage {
										model_type: model_type.clone(),
										record_uuid: uuid,
										device_id: source_device_id,
										data: failed_data,
										timestamp: record_timestamp,
									};

									self.peer_sync
										.dependency_tracker()
										.add_dependency(
											missing_uuid,
											super::state::BufferedUpdate::StateChange(state_change),
										)
										.await;
								}
							}
						}

						result.succeeded
					} else {
						record_data
					};

					// Apply updates via registry with FKs already resolved
					// The idempotent map_sync_json_to_local in apply_state_change will skip already-resolved FKs
					// Collect successfully applied UUIDs for batch event emission
					let mut applied_uuids = Vec::new();

					for data in processed_data {
						// Extract UUID before moving data
						let record_uuid = data
							.get("uuid")
							.and_then(|v| v.as_str())
							.and_then(|s| Uuid::parse_str(s).ok());

						crate::infra::sync::registry::apply_state_change(
							&model_type,
							data,
							db.clone(),
						)
						.await
						.map_err(|e| anyhow::anyhow!("{}", e))?;

						// Track successfully applied UUID for batch event emission
						if let Some(uuid) = record_uuid {
							applied_uuids.push(uuid);
						}

						// After successfully applying, resolve any records waiting for this one
						// (e.g., child entries waiting for their parent entry)
						if let Some(uuid) = record_uuid {
							let waiting_updates =
								self.peer_sync.dependency_tracker().resolve(uuid).await;

							if !waiting_updates.is_empty() {
								tracing::debug!(
									resolved_uuid = %uuid,
									model_type = %model_type,
									waiting_count = waiting_updates.len(),
									"Resolving dependent records after device-owned backfill"
								);

								for update in waiting_updates {
									if let super::state::BufferedUpdate::StateChange(
										dependent_change,
									) = update
									{
										if let Err(e) = self
											.peer_sync
											.apply_state_change(dependent_change.clone())
											.await
										{
											// If still failing (e.g., grandparent missing), re-queue
											tracing::debug!(
												error = %e,
												record_uuid = %dependent_change.record_uuid,
												"Dependent record still has missing deps, will retry"
											);
										}
									}
								}
							}
						}
					}

					// Emit resource events in batch for UI reactivity
					if !applied_uuids.is_empty() {
						let resource_manager = crate::domain::ResourceManager::new(
							db.clone(),
							self.peer_sync.event_bus().clone(),
						);

						if let Err(e) = resource_manager
							.emit_batch_resource_events(&model_type, applied_uuids)
							.await
						{
							warn!(
								model_type = %model_type,
								error = %e,
								"Failed to emit batch resource events after backfill"
							);
						}
					}

					// Record data volume metrics
					self.metrics
						.record_entries_synced(&model_type, records_count)
						.await;

					// Feed batch aggregator for event logging
					self.batch_aggregator
						.add_records(model_type.clone(), records_count, Some(peer))
						.await;

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

		Ok((current_checkpoint, max_timestamp))
	}

	/// Backfill shared resources
	async fn backfill_shared_resources(
		&self,
		peer: Uuid,
	) -> Result<Option<crate::infra::sync::HLC>> {
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
			// NOTE: we keep hitting this almost always, I don't think I've ever seen the above log. This concerns me greatly.
			info!("Backfilling all shared resources");
		}

		// Request shared changes from peer in batches (can be 100k+ records)
		let mut last_hlc = since_hlc;
		let mut total_applied = 0;
		let mut last_progress_log = 0;
		let mut received_any_data = false;

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

				// Track max HLC for ACK (critical for pruning)
				let max_hlc_in_batch = entries.last().map(|e| e.hlc);

				// IMPORTANT: Apply current_state snapshot FIRST (before HLC entries)
				// The snapshot contains base data that peer log entries may depend on
				// This prevents FK constraint errors when peer log references snapshot data
				if let Some(state) = current_state {
					received_any_data = true;
					if let Some(state_map) = state.as_object() {
						// Get dependency-ordered list of models to prevent FK violations
						// CRITICAL: Must apply parent models before children (e.g., user_metadata before user_metadata_tag)
						let sync_order =
							match crate::infra::sync::registry::compute_registry_sync_order().await
							{
								Ok(order) => order,
								Err(e) => {
									warn!("Failed to compute sync order, using unordered: {}", e);
									// Fallback to unordered if dependency graph fails
									state_map.keys().map(|k| k.clone()).collect::<Vec<_>>()
								}
							};

						// Apply snapshot records in dependency order
						for model_type in sync_order {
							// Skip if model not in snapshot
							let records_value = match state_map.get(&model_type) {
								Some(val) => val,
								None => continue,
							};

							if let Some(records_array) = records_value.as_array() {
								debug!(
									model_type = %model_type,
									count = records_array.len(),
									"Applying current state snapshot for pre-sync data"
								);

								// Collect successfully applied UUIDs for batch event emission
								let mut applied_snapshot_uuids: Vec<Uuid> = Vec::new();

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
														let mut hlc_gen = self
															.peer_sync
															.hlc_generator()
															.lock()
															.await;
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
													match crate::infra::sync::registry::apply_shared_change(entry.clone(), db.clone()).await {
														Ok(()) => {
															// Track successfully applied UUID for batch event emission
															applied_snapshot_uuids.push(record_uuid);

															// Resolve any state changes waiting for this shared resource
															let waiting_updates = self
																.peer_sync
																.dependency_tracker()
																.resolve(record_uuid)
																.await;

															if !waiting_updates.is_empty() {
																tracing::debug!(
																	resolved_uuid = %record_uuid,
																	model_type = %model_type,
																	waiting_count = waiting_updates.len(),
																	"Resolving dependencies after current_state snapshot"
																);

																for update in waiting_updates {
																	match update {
																		super::state::BufferedUpdate::StateChange(dependent_change) => {
																			if let Err(e) = self
																				.peer_sync
																				.apply_state_change(dependent_change.clone())
																				.await
																			{
																				tracing::warn!(
																					error = %e,
																					record_uuid = %dependent_change.record_uuid,
																					"Failed to apply dependent state change after current_state snapshot"
																				);
																			}
																		}
																		super::state::BufferedUpdate::SharedChange(dependent_entry) => {
																			// Retry the shared change now that its dependency exists
																			let entry_clone = dependent_entry.clone();
																			let db = self.peer_sync.db().clone();
																			if let Err(e) = crate::infra::sync::registry::apply_shared_change(entry_clone, db).await {
																				tracing::warn!(
																					error = %e,
																					record_uuid = %dependent_entry.record_uuid,
																					"Failed to apply dependent shared change after current_state snapshot"
																				);
																			}
																		}
																	}
																}
															}
														}
														Err(e) => {
															let error_str = e.to_string();

															// Check if this is a FK dependency error
															if error_str.contains("Sync dependency missing")
																|| error_str.contains("FOREIGN KEY constraint failed")
															{
																// Try to extract the missing UUID from the error message
																if let Some(missing_uuid) =
																	super::dependency::extract_missing_dependency_uuid(&error_str)
																{
																	tracing::debug!(
																		record_uuid = %record_uuid,
																		model_type = %model_type,
																		missing_uuid = %missing_uuid,
																		"Snapshot record has missing FK dependency, buffering for retry"
																	);

																	// Buffer this shared change for retry when dependency arrives
																	self.peer_sync
																		.dependency_tracker()
																		.add_dependency(
																			missing_uuid,
																			super::state::BufferedUpdate::SharedChange(
																				entry.clone(),
																			),
																		)
																		.await;

																	continue; // Skip to next record
																} else {
																	// FK error but can't extract UUID
																	let fk_mappings = crate::infra::sync::registry::get_fk_mappings(&model_type);
																	let uuid_fields: Vec<String> = if let Some(obj) = data.as_object() {
																		obj.keys()
																			.filter(|k| k.ends_with("_uuid"))
																			.map(|k| {
																				let value = obj.get(k).and_then(|v| v.as_str()).unwrap_or("null");
																				format!("{}={}", k, value)
																			})
																			.collect()
																	} else {
																		vec![]
																	};

																	warn!(
																		model_type = %model_type,
																		uuid = %record_uuid,
																		error = %e,
																		fk_mappings = ?fk_mappings,
																		uuid_fields = ?uuid_fields,
																		"Failed to apply current_state snapshot record - FK constraint failed but cannot extract missing UUID"
																	);

																	continue; // Skip this record
																}
															}

															// Non-dependency error - this is unexpected, log but continue
															warn!(
																model_type = %model_type,
																uuid = %record_uuid,
																error = %e,
																"Failed to apply current_state snapshot record - unexpected error"
															);
														}
													}
												}
											}
										}
									}
								}

								// Emit batch resource events for all successfully applied snapshot records
								if !applied_snapshot_uuids.is_empty() {
									let db = self.peer_sync.db().clone();
									let resource_manager = crate::domain::ResourceManager::new(
										db.clone(),
										self.peer_sync.event_bus().clone(),
									);

									if let Err(e) = resource_manager
										.emit_batch_resource_events(&model_type, applied_snapshot_uuids)
										.await
									{
										warn!(
											model_type = %model_type,
											error = %e,
											"Failed to emit batch resource events after snapshot application"
										);
									}
								}
							}
						}
					}
				}

				// Now apply HLC-ordered peer log entries (after snapshot dependencies are available)
				for entry in &entries {
					// Attempt to apply the shared change
					// If it fails due to missing FK dependency, buffer it for retry
					match self.log_handler.handle_shared_change(entry.clone()).await {
						Ok(()) => {
							// Success - resolve any waiting dependencies
						}
						Err(e) => {
							let error_str = e.to_string();

							// Check if this is a FK dependency error (from FK mapping or SQLite constraint)
							if error_str.contains("Sync dependency missing")
								|| error_str.contains("FOREIGN KEY constraint failed")
							{
								// Try to extract the missing UUID from the error message
								if let Some(missing_uuid) =
									super::dependency::extract_missing_dependency_uuid(&error_str)
								{
									tracing::debug!(
										record_uuid = %entry.record_uuid,
										model_type = %entry.model_type,
										missing_uuid = %missing_uuid,
										"Shared resource has missing FK dependency, buffering for retry"
									);

									// Buffer this shared change for retry when dependency arrives
									self.peer_sync
										.dependency_tracker()
										.add_dependency(
											missing_uuid,
											super::state::BufferedUpdate::SharedChange(
												entry.clone(),
											),
										)
										.await;

									continue; // Skip to next entry
								} else {
									// FK error but can't extract UUID (raw SQLite error)
									// Extract diagnostic information for troubleshooting
									let fk_mappings = crate::infra::sync::registry::get_fk_mappings(&entry.model_type);

									// Extract UUID fields from entry data to show which FKs are present
									let uuid_fields: Vec<String> = if let Some(obj) = entry.data.as_object() {
										obj.keys()
											.filter(|k| k.ends_with("_uuid"))
											.map(|k| {
												let value = obj.get(k).and_then(|v| v.as_str()).unwrap_or("null");
												format!("{}={}", k, value)
											})
											.collect()
									} else {
										vec![]
									};

									// Log comprehensive diagnostic information
									tracing::info!(
										error = %e,
										record_uuid = %entry.record_uuid,
										model_type = %entry.model_type,
										hlc = %entry.hlc,
										change_type = ?entry.change_type,
										fk_mappings = ?fk_mappings,
										uuid_fields = ?uuid_fields,
										"FK constraint failed during shared resource backfill - cannot extract missing UUID from SQLite error. \
										This typically means: (1) parent was deleted before backfill started, (2) peer log has orphaned records, \
										(3) snapshot query failed for a dependency model, or (4) HLC ordering issue. \
										Record will be skipped and retried in next backfill attempt."
									);

									// Skip this record - if it's in the snapshot dependencies, it will be resolved
									// If not, it will be re-requested in next backfill attempt
									continue;
								}
							}

							// Non-dependency error - fail backfill
							return Err(e);
						}
					}

					// Resolve any state changes waiting for this shared resource
					// This handles cross-type dependencies (e.g., entries waiting for content_identities)
					let waiting_updates = self
						.peer_sync
						.dependency_tracker()
						.resolve(entry.record_uuid)
						.await;

					if !waiting_updates.is_empty() {
						tracing::debug!(
							resolved_uuid = %entry.record_uuid,
							model_type = %entry.model_type,
							waiting_count = waiting_updates.len(),
							"Resolving dependencies after shared resource backfill"
						);

						for update in waiting_updates {
							match update {
								super::state::BufferedUpdate::StateChange(dependent_change) => {
									if let Err(e) = self
										.peer_sync
										.apply_state_change(dependent_change.clone())
										.await
									{
										tracing::warn!(
											error = %e,
											record_uuid = %dependent_change.record_uuid,
											"Failed to apply dependent state change after shared resource backfill"
										);
									}
								}
								super::state::BufferedUpdate::SharedChange(dependent_entry) => {
									// Retry the shared change now that its dependency exists
									if let Err(e) = self
										.log_handler
										.handle_shared_change(dependent_entry.clone())
										.await
									{
										tracing::warn!(
											error = %e,
											record_uuid = %dependent_entry.record_uuid,
											"Failed to apply dependent shared change after parent resolved"
										);
									}
								}
							}
						}
					}
				}

				total_applied += batch_size;

				// Send ACK back to peer for pruning (matches real-time path behavior)
				// This allows the sender to prune acknowledged changes from their sync.db
				if let Some(up_to_hlc) = max_hlc_in_batch {
					// Check if these changes were created by us
					let change_creator = up_to_hlc.device_id;

					if change_creator == self.device_id {
						// These are our own changes (synced to peer, now coming back during backfill)
						// Don't ACK them - we don't need to tell ourselves we've seen our own changes
						tracing::debug!(
							hlc = %up_to_hlc,
							batch_size = batch_size,
							"Skipping self-ACK during backfill (changes created by us)"
						);
					} else {
						// These are peer's changes - send ACK so they can prune
						let ack_message = SyncMessage::AckSharedChanges {
							library_id: self.library_id,
							from_device: self.device_id,
							up_to_hlc,
						};

						// Send ACK via network (best-effort, don't fail backfill if ACK fails)
						if let Err(e) = self
							.peer_sync
							.network()
							.send_sync_message(peer, ack_message)
							.await
						{
							warn!(
								peer = %peer,
								hlc = %up_to_hlc,
								error = %e,
								"Failed to send ACK for shared changes (pruning may be delayed)"
							);
						} else {
							debug!(
								peer = %peer,
								hlc = %up_to_hlc,
								batch_size = batch_size,
								"Sent ACK for peer's shared changes"
							);
						}
					}
				}

				// Record metrics
				self.metrics.record_backfill_pagination_round();
				self.metrics
					.record_entries_synced("shared", batch_size as u64)
					.await;

				// Feed batch aggregator for event logging
				self.batch_aggregator
					.add_records(
						"shared_resources".to_string(),
						batch_size as u64,
						Some(peer),
					)
					.await;

				// Log progress every 10,000 records for large backfills
				if total_applied >= last_progress_log + 10_000 {
					debug!(
						total_applied = total_applied,
						batch_size = batch_size,
						"Backfilling shared resources - progress update"
					);
					last_progress_log = total_applied;
				} else {
					debug!(
						"Applied {} shared changes (total: {})",
						batch_size, total_applied
					);
				}

				// Update cursor to last HLC for next batch
				if let Some(last_entry) = entries.last() {
					last_hlc = Some(last_entry.hlc);
				}

				// Continue if there are more entries
				if !has_more || batch_size == 0 {
					break;
				}
			} else {
				break;
			}
		}

		debug!(
			"Shared resources backfill complete (total: {} entries)",
			total_applied
		);

		// Return the max HLC from received data for accurate watermark tracking
		//
		// Special case: If we received snapshot data but no peer log entries,
		// generate a watermark HLC representing "synced up to now".
		//
		// This happens on initial sync when the peer's log is empty/pruned and they
		// send only a current_state snapshot. Snapshots contain raw data without HLC
		// metadata (protocol limitation - timestamps are discarded during serialization).
		//
		// We generate an HLC with our device_id and current timestamp. This works because:
		// - HLC comparison uses timestamp first, device_id only as tiebreaker
		// - Next sync will request entries with timestamp > this watermark
		// - Any peer entries created after now will have higher timestamp regardless of device_id
		if last_hlc.is_none() && received_any_data {
			let watermark_hlc = self.peer_sync.hlc_generator().lock().await.next();
			debug!(
				watermark_hlc = %watermark_hlc,
				"Snapshot-only sync (no peer log entries), using current time as watermark"
			);
			Ok(Some(watermark_hlc))
		} else {
			Ok(last_hlc)
		}
	}

	/// Request state batch from peer
	///
	/// Sends a StateRequest via bidirectional stream and waits for StateResponse.
	/// Also measures RTT for peer latency metrics.
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
			device_id: Some(peer), // Request only entries owned by the peer
			since,
			checkpoint,
			batch_size,
		};

		// Measure RTT for peer latency tracking
		let start = std::time::Instant::now();

		// Use send_sync_request which handles bidirectional stream and response
		let response = self
			.peer_sync
			.network()
			.send_sync_request(peer, request)
			.await?;

		// Record peer RTT
		let rtt_ms = start.elapsed().as_millis() as f32;
		self.metrics.record_peer_rtt(peer, rtt_ms).await;

		Ok(response)
	}

	/// Request shared changes from peer
	///
	/// Sends a SharedChangeRequest via bidirectional stream and waits for SharedChangeResponse.
	/// Also measures RTT for peer latency metrics.
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

		// Measure RTT for peer latency tracking
		let start = std::time::Instant::now();

		// Use send_sync_request which handles bidirectional stream and response
		let response = self
			.peer_sync
			.network()
			.send_sync_request(peer, request)
			.await?;

		// Record peer RTT
		let rtt_ms = start.elapsed().as_millis() as f32;
		self.metrics.record_peer_rtt(peer, rtt_ms).await;

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
		peer: Uuid,
		final_state_checkpoint: Option<String>,
		max_shared_hlc: Option<crate::infra::sync::HLC>,
	) -> Result<()> {
		self.peer_sync
			.set_initial_watermarks(peer, final_state_checkpoint, max_shared_hlc)
			.await
	}
}
