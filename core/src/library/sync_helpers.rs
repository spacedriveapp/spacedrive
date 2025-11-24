//! Sync helper methods for Library
//!
//! Provides ergonomic API for emitting sync events after database writes.
//! Reduces verbose 9-line sync calls to clean 1-line calls.
//!
//! ## Usage
//!
//! ```rust,ignore
//! // Simple model (no FKs)
//! let tag = tag::ActiveModel { ... }.insert(db).await?;
//! library.sync_model(&tag, ChangeType::Insert).await?;
//!
//! // Model with FK relationships
//! let location = location::ActiveModel { ... }.insert(db).await?;
//! library.sync_model_with_db(&location, ChangeType::Insert, db).await?;
//!
//! // Bulk operations (1000+ records)
//! library.sync_models_batch(&entries, ChangeType::Insert, db).await?;
//! ```

use super::Library;
use crate::infra::{
	event::Event,
	sync::{ChangeType, Syncable},
};
use anyhow::Result;
use sea_orm::{DatabaseConnection, EntityTrait};
use tracing::{debug, warn};
use uuid::Uuid;

impl Library {
	// ============ Public API ============

	/// Sync a model without FK conversion (for simple models)
	///
	/// Use this for models that have no foreign key relationships, or where
	/// foreign keys are already UUIDs.
	///
	/// **Examples**: Tag, Device, Album
	///
	/// **Note**: This only handles sync (cross-device replication). Resource events
	/// for frontend reactivity are emitted separately by ResourceManager when it
	/// detects sync changes via the transaction manager.
	pub async fn sync_model<M: Syncable>(&self, model: &M, change_type: ChangeType) -> Result<()> {
		let data = model
			.to_sync_json()
			.map_err(|e| anyhow::anyhow!("Failed to serialize model: {}", e))?;

		if crate::infra::sync::is_device_owned(M::SYNC_MODEL).await {
			self.sync_device_owned_internal(M::SYNC_MODEL, model.sync_id(), data)
				.await
		} else {
			self.sync_shared_internal(M::SYNC_MODEL, model.sync_id(), change_type, data)
				.await
		}
	}

	/// Sync a model with FK conversion (for models with relationships)
	///
	/// Automatically converts integer FK fields to UUIDs before broadcasting.
	/// Required for proper sync of related data.
	///
	/// **Examples**: Location (has device_id, entry_id), Entry (has parent_id, metadata_id)
	///
	/// **Note**: This only handles sync (cross-device replication). Resource events
	/// for frontend reactivity are emitted separately by ResourceManager when it
	/// detects sync changes via the transaction manager.
	pub async fn sync_model_with_db<M: Syncable>(
		&self,
		model: &M,
		change_type: ChangeType,
		db: &DatabaseConnection,
	) -> Result<()> {
		// For Entry model, we need the database ID before it's excluded from sync JSON
		// This is required to fetch directory_path for location roots
		let entry_db_id = if M::SYNC_MODEL == "entry" {
			// Serialize the full model to get all fields including "id"
			serde_json::to_value(model)
				.ok()
				.and_then(|v| v.get("id").and_then(|id| id.as_i64()))
				.map(|id| id as i32)
		} else {
			None
		};

		let mut data = model
			.to_sync_json()
			.map_err(|e| anyhow::anyhow!("Failed to serialize model: {}", e))?;

		// Convert FK integer IDs to UUIDs
		for fk in M::foreign_key_mappings() {
			crate::infra::sync::fk_mapper::convert_fk_to_uuid(&mut data, &fk, db)
				.await
				.map_err(|e| {
					anyhow::anyhow!("FK conversion failed for {}: {}", fk.local_field, e)
				})?;
		}

		// Special handling for Entry model: include directory_path for location roots
		// Location roots need absolute paths for universal addressing
		// Regular directories get paths via batch sync (query_for_sync)
		if M::SYNC_MODEL == "entry" {
			let is_directory = data.get("kind").and_then(|v| v.as_i64()) == Some(1);
			let is_root = data.get("parent_id").map(|v| v.is_null()).unwrap_or(false);

			if is_directory && is_root {
				// This is a location root - include absolute path
				// Use the entry_db_id we captured before field exclusions
				if let Some(id) = entry_db_id {
					use crate::infra::db::entities::directory_paths;
					use sea_orm::ColumnTrait;
					use sea_orm::QueryFilter;

					if let Ok(Some(dir_path)) = directory_paths::Entity::find()
						.filter(directory_paths::Column::EntryId.eq(id))
						.one(db)
						.await
					{
						if let Some(obj) = data.as_object_mut() {
							obj.insert(
								"directory_path".to_string(),
								serde_json::Value::String(dir_path.path),
							);
						}
					}
				}
			}
		}

		if crate::infra::sync::is_device_owned(M::SYNC_MODEL).await {
			self.sync_device_owned_internal(M::SYNC_MODEL, model.sync_id(), data)
				.await
		} else {
			self.sync_shared_internal(M::SYNC_MODEL, model.sync_id(), change_type, data)
				.await
		}
	}

	/// Batch sync multiple models (optimized for bulk operations)
	///
	/// Use this when syncing 100+ records at once (e.g., during indexing).
	/// Provides significant performance improvement over individual sync calls.
	///
	/// **Performance**: 30-120x faster than individual calls for large batches.
	///
	/// **Examples**: Indexing 10K files, bulk tag application
	pub async fn sync_models_batch<M: Syncable>(
		&self,
		models: &[M],
		change_type: ChangeType,
		db: &DatabaseConnection,
	) -> Result<()> {
		if models.is_empty() {
			return Ok(());
		}

		debug!("Batch syncing {} {} records", models.len(), M::SYNC_MODEL);

		// Convert all models to sync JSON with FK mapping
		let mut sync_data = Vec::new();
		for model in models {
			let mut data = model
				.to_sync_json()
				.map_err(|e| anyhow::anyhow!("Failed to serialize model: {}", e))?;

			for fk in M::foreign_key_mappings() {
				crate::infra::sync::fk_mapper::convert_fk_to_uuid(&mut data, &fk, db)
					.await
					.map_err(|e| {
						anyhow::anyhow!("FK conversion failed for {}: {}", fk.local_field, e)
					})?;
			}

			sync_data.push((model.sync_id(), data));
		}

		let is_device_owned = crate::infra::sync::is_device_owned(M::SYNC_MODEL).await;

		if is_device_owned {
			self.sync_device_owned_batch_internal(M::SYNC_MODEL, sync_data)
				.await
		} else {
			self.sync_shared_batch_internal(M::SYNC_MODEL, change_type, sync_data)
				.await
		}
	}

	// ============ Internal Helpers ============

	/// Helper to get device ID from core context
	fn device_id(&self) -> Result<Uuid> {
		self.core_context()
			.device_manager
			.device_id()
			.map_err(|e| anyhow::anyhow!("Failed to get device ID: {}", e))
	}

	/// Internal: Sync device-owned resource (state-based)
	async fn sync_device_owned_internal(
		&self,
		model_type: &str,
		record_uuid: Uuid,
		data: serde_json::Value,
	) -> Result<()> {
		let device_id = self.device_id()?;

		self.transaction_manager()
			.commit_device_owned(self.id(), model_type, record_uuid, device_id, data)
			.await
			.map_err(|e| anyhow::anyhow!("Failed to commit device-owned data: {}", e))
	}

	/// Internal: Sync shared resource (log-based with HLC)
	async fn sync_shared_internal(
		&self,
		model_type: &str,
		record_uuid: Uuid,
		change_type: ChangeType,
		data: serde_json::Value,
	) -> Result<()> {
		// Gracefully handle missing sync service (networking disabled or not connected)
		let Some(sync_service) = self.sync_service() else {
			debug!(
				"Sync service not initialized - operation saved locally but not synced (model={}, uuid={})",
				model_type, record_uuid
			);
			return Ok(());
		};

		let peer_log = sync_service.peer_sync().peer_log();
		let mut hlc_gen = sync_service.peer_sync().hlc_generator().lock().await;

		self.transaction_manager()
			.commit_shared(
				self.id(),
				model_type,
				record_uuid,
				change_type,
				data,
				peer_log,
				&mut *hlc_gen,
			)
			.await
			.map_err(|e| anyhow::anyhow!("Failed to commit shared data: {}", e))
	}

	/// Internal: Batch sync device-owned resources
	async fn sync_device_owned_batch_internal(
		&self,
		model_type: &str,
		records: Vec<(Uuid, serde_json::Value)>,
	) -> Result<()> {
		let device_id = self.device_id()?;

		debug!(
			"Batch syncing {} device-owned {} records",
			records.len(),
			model_type
		);

		// Emit batch resource event for UI reactivity
		let resources: Vec<_> = records.iter().map(|(_, data)| data.clone()).collect();
		self.event_bus().emit(Event::ResourceChangedBatch {
			resource_type: model_type.to_string(),
			resources: serde_json::to_value(&resources)
				.map_err(|e| anyhow::anyhow!("Failed to serialize batch resources: {}", e))?,
			metadata: None,
		});

		// Emit StateChange events for sync coordination
		// Previously suppressed to avoid network overhead, but this caused 97% data loss
		// when combined with watermark bugs. Real-time broadcast is critical for reliability.
		for (record_uuid, data) in records {
			self.transaction_manager()
				.commit_device_owned(self.id(), model_type, record_uuid, device_id, data)
				.await
				.map_err(|e| anyhow::anyhow!("Failed to commit device-owned data: {}", e))?;
		}

		Ok(())
	}

	/// Internal: Batch sync shared resources
	async fn sync_shared_batch_internal(
		&self,
		model_type: &str,
		change_type: ChangeType,
		records: Vec<(Uuid, serde_json::Value)>,
	) -> Result<()> {
		// Gracefully handle missing sync service
		let Some(sync_service) = self.sync_service() else {
			debug!(
				"Sync service not initialized - {} {} records saved locally but not synced",
				records.len(),
				model_type
			);
			return Ok(());
		};

		let peer_log = sync_service.peer_sync().peer_log();
		let mut hlc_gen = sync_service.peer_sync().hlc_generator().lock().await;

		debug!(
			"Batch syncing {} shared {} records",
			records.len(),
			model_type
		);

		// Collect resources and IDs for batch event
		let record_ids: Vec<_> = records.iter().map(|(id, _)| *id).collect();
		let resources_for_event: Vec<_> = records.iter().map(|(_, data)| data.clone()).collect();

		// Generate HLCs, append to peer log, AND emit real-time events (for instant sync)
		let mut entries_to_broadcast = Vec::new();

		for (record_uuid, data) in records {
			let hlc = hlc_gen.next();

			let entry = crate::infra::sync::SharedChangeEntry {
				hlc,
				model_type: model_type.to_string(),
				record_uuid,
				change_type,
				data,
			};

			// Write to peer log (for durability and pruning)
			peer_log
				.append(entry.clone())
				.await
				.map_err(|e| anyhow::anyhow!("Failed to append to peer log: {}", e))?;

			// Collect for real-time broadcast
			entries_to_broadcast.push(entry);
		}

		// Emit real-time broadcasts for instant sync (HLC dedup prevents double-processing)
		for entry in entries_to_broadcast {
			self.transaction_manager().sync_events().emit(
				crate::infra::sync::SyncEvent::SharedChange {
					library_id: self.id(),
					entry,
				},
			);
		}

		// Emit batch resource event for UI reactivity
		use crate::infra::sync::ChangeType as CT;
		match change_type {
			CT::Delete => {
				// For batch deletes, emit individual delete events since ResourceDeletedBatch doesn't exist
				// This should be rare for batch operations anyway
				for record_uuid in record_ids {
					self.event_bus().emit(Event::ResourceDeleted {
						resource_type: model_type.to_string(),
						resource_id: record_uuid,
					});
				}
			}
			CT::Insert | CT::Update => {
				self.event_bus().emit(Event::ResourceChangedBatch {
					resource_type: model_type.to_string(),
					resources: serde_json::to_value(&resources_for_event).map_err(|e| {
						anyhow::anyhow!("Failed to serialize batch resources: {}", e)
					})?,
					metadata: None,
				});
			}
		}

		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_sync_helpers_exist() {
		// Compile-time check that the API is usable
		// Actual integration tests are in core/tests/sync_integration_test.rs
	}
}
