//! Foreign Key UUID Mapping for Sync
//!
//! Handles automatic conversion between local integer FKs and global UUIDs for sync.
//!
//! ## The Problem
//!
//! Auto-incrementing integer PKs are local to each database:
//! - Device A has device_id=1 for Device A, device_id=2 for Device B
//! - Device B has device_id=1 for Device B, device_id=2 for Device A
//!
//! We cannot sync integer IDs directly - they mean different things on each device.
//!
//! ## The Solution
//!
//! Sync protocol uses UUIDs exclusively. This module provides:
//! 1. `to_sync_json()` - Converts local integer FKs → UUIDs before sending
//! 2. `map_uuids_to_local_ids()` - Converts UUIDs → local integer FKs on receive
//!
//! ## Usage
//!
//! Models just declare their FKs, the rest is automatic:
//!
//! ```rust
//! impl Syncable for location::Model {
//!     fn foreign_key_mappings() -> Vec<FKMapping> {
//!         vec![
//!             FKMapping::new("device_id", "devices"),
//!             FKMapping::new("entry_id", "entries"),
//!         ]
//!     }
//!
//!     // to_sync_json() - uses default implementation (automatic!)
//!
//!     async fn apply_state_change(data: Value, db: &DatabaseConnection) -> Result<()> {
//!         let data = map_sync_json_to_local(data, Self::foreign_key_mappings(), db).await?;
//!         let model: Model = serde_json::from_value(data)?;
//!         upsert_by_uuid(model).await?;
//!         Ok(())
//!     }
//! }
//! ```

use std::collections::{HashMap, HashSet};

use crate::infra::db::entities;
use anyhow::{anyhow, Result};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde_json::{json, Value};
use uuid::Uuid;

/// Foreign key mapping declaration
#[derive(Debug, Clone)]
pub struct FKMapping {
	/// Field name in the model (e.g., "device_id")
	pub local_field: &'static str,

	/// Target table name (e.g., "devices")
	pub target_table: &'static str,
}

impl FKMapping {
	pub fn new(local_field: &'static str, target_table: &'static str) -> Self {
		Self {
			local_field,
			target_table,
		}
	}

	/// Get the UUID field name for sync JSON
	/// Example: "device_id" → "device_uuid"
	pub fn uuid_field_name(&self) -> String {
		format!("{}_uuid", self.local_field.trim_end_matches("_id"))
	}
}

/// Convert local integer FK to UUID for sync
///
/// Modifies JSON in place:
/// - Looks up the UUID for the integer FK
/// - Adds a new field with UUID (e.g., "device_uuid")
/// - Removes the original integer field (e.g., "device_id")
pub async fn convert_fk_to_uuid(
	json: &mut Value,
	fk: &FKMapping,
	db: &DatabaseConnection,
) -> Result<()> {
	// Extract the local integer ID
	let local_id = match json.get(fk.local_field).and_then(|v| v.as_i64()) {
		Some(id) => id as i32,
		None => {
			// Field might be null (e.g., parent_id for root entries)
			if json
				.get(fk.local_field)
				.map(|v| v.is_null())
				.unwrap_or(false)
			{
				// Add null UUID field and return
				json[fk.uuid_field_name()] = Value::Null;
				return Ok(());
			}
			return Err(anyhow!(
				"Missing or invalid FK field '{}' in sync data",
				fk.local_field
			));
		}
	};

	// Look up UUID from target table
	let uuid = lookup_uuid_for_local_id(fk.target_table, local_id, db).await?;

	// Add UUID field to JSON
	json[fk.uuid_field_name()] = json!(uuid.to_string());

	// Remove integer ID field (we only sync UUIDs)
	if let Some(obj) = json.as_object_mut() {
		obj.remove(fk.local_field);
	}

	Ok(())
}

/// Look up UUID for a local integer ID in any table
async fn lookup_uuid_for_local_id(
	table: &str,
	local_id: i32,
	db: &DatabaseConnection,
) -> Result<Uuid> {
	match table {
		"devices" => {
			let device = entities::device::Entity::find_by_id(local_id)
				.one(db)
				.await?
				.ok_or_else(|| anyhow!("Device with id={} not found", local_id))?;
			Ok(device.uuid)
		}
		"entries" => {
			let entry = entities::entry::Entity::find_by_id(local_id)
				.one(db)
				.await?
				.ok_or_else(|| anyhow!("Entry with id={} not found", local_id))?;
			entry.uuid.ok_or_else(|| {
				anyhow!("Entry id={} has no UUID (data consistency error)", local_id)
			})
		}
		"locations" => {
			let location = entities::location::Entity::find_by_id(local_id)
				.one(db)
				.await?
				.ok_or_else(|| anyhow!("Location with id={} not found", local_id))?;
			Ok(location.uuid)
		}
		"user_metadata" => {
			let metadata = entities::user_metadata::Entity::find_by_id(local_id)
				.one(db)
				.await?
				.ok_or_else(|| anyhow!("UserMetadata with id={} not found", local_id))?;
			Ok(metadata.uuid)
		}
		"content_identities" => {
			let content = entities::content_identity::Entity::find_by_id(local_id)
				.one(db)
				.await?
				.ok_or_else(|| anyhow!("ContentIdentity with id={} not found", local_id))?;
			content
				.uuid
				.ok_or_else(|| anyhow!("ContentIdentity id={} has no UUID", local_id))
		}
		"volumes" => {
			let volume = entities::volume::Entity::find_by_id(local_id)
				.one(db)
				.await?
				.ok_or_else(|| anyhow!("Volume with id={} not found", local_id))?;
			Ok(volume.uuid)
		}
		"collection" => {
			let collection = entities::collection::Entity::find_by_id(local_id)
				.one(db)
				.await?
				.ok_or_else(|| anyhow!("Collection with id={} not found", local_id))?;
			Ok(collection.uuid)
		}
		"tag" => {
			let tag = entities::tag::Entity::find_by_id(local_id)
				.one(db)
				.await?
				.ok_or_else(|| anyhow!("Tag with id={} not found", local_id))?;
			Ok(tag.uuid)
		}
		_ => Err(anyhow!("Unknown table for FK mapping: {}", table)),
	}
}

/// Batch look up UUIDs for multiple local integer IDs in any table
///
/// Returns a HashMap mapping local_id -> UUID for all found records.
/// Records not found are omitted from the result map (caller must handle missing entries).
///
/// This function performs a single SQL query with WHERE id IN (...) instead of
/// N individual queries.
pub async fn batch_lookup_uuids_for_local_ids(
	table: &str,
	local_ids: HashSet<i32>,
	db: &DatabaseConnection,
) -> Result<HashMap<i32, Uuid>> {
	if local_ids.is_empty() {
		return Ok(HashMap::new());
	}

	let id_vec: Vec<i32> = local_ids.into_iter().collect();

	match table {
		"devices" => {
			let records = entities::device::Entity::find()
				.filter(entities::device::Column::Id.is_in(id_vec))
				.all(db)
				.await?;
			Ok(records.into_iter().map(|r| (r.id, r.uuid)).collect())
		}
		"entries" => {
			let records = entities::entry::Entity::find()
				.filter(entities::entry::Column::Id.is_in(id_vec))
				.all(db)
				.await?;
			let mut map = HashMap::new();
			for r in records {
				if let Some(uuid) = r.uuid {
					map.insert(r.id, uuid);
				}
			}
			Ok(map)
		}
		"locations" => {
			let records = entities::location::Entity::find()
				.filter(entities::location::Column::Id.is_in(id_vec))
				.all(db)
				.await?;
			Ok(records.into_iter().map(|r| (r.id, r.uuid)).collect())
		}
		"user_metadata" => {
			let records = entities::user_metadata::Entity::find()
				.filter(entities::user_metadata::Column::Id.is_in(id_vec))
				.all(db)
				.await?;
			Ok(records.into_iter().map(|r| (r.id, r.uuid)).collect())
		}
		"content_identities" => {
			let records = entities::content_identity::Entity::find()
				.filter(entities::content_identity::Column::Id.is_in(id_vec))
				.all(db)
				.await?;
			let mut map = HashMap::new();
			for r in records {
				if let Some(uuid) = r.uuid {
					map.insert(r.id, uuid);
				}
			}
			Ok(map)
		}
		"volumes" => {
			let records = entities::volume::Entity::find()
				.filter(entities::volume::Column::Id.is_in(id_vec))
				.all(db)
				.await?;
			Ok(records.into_iter().map(|r| (r.id, r.uuid)).collect())
		}
		"collection" => {
			let records = entities::collection::Entity::find()
				.filter(entities::collection::Column::Id.is_in(id_vec))
				.all(db)
				.await?;
			Ok(records.into_iter().map(|r| (r.id, r.uuid)).collect())
		}
		"tag" => {
			let records = entities::tag::Entity::find()
				.filter(entities::tag::Column::Id.is_in(id_vec))
				.all(db)
				.await?;
			Ok(records.into_iter().map(|r| (r.id, r.uuid)).collect())
		}
		_ => Err(anyhow!("Unknown table for FK mapping: {}", table)),
	}
}

/// Convert UUIDs back to local integer IDs for database insertion
///
/// Modifies JSON in place:
/// - Looks up local ID for each UUID FK
/// - Replaces UUID field with integer ID field
/// - Removes UUID field
///
/// This function is idempotent - if FK is already resolved (local_field exists, uuid_field doesn't),
/// it will skip processing. This allows batch FK resolution followed by per-record application.
pub async fn map_sync_json_to_local(
	mut data: Value,
	mappings: Vec<FKMapping>,
	db: &DatabaseConnection,
) -> Result<Value> {
	for fk in mappings {
		let uuid_field = fk.uuid_field_name();

		// Check if FK is already resolved (idempotent behavior)
		// If uuid_field doesn't exist and local_field exists, FK was already resolved
		let uuid_value = data.get(&uuid_field);
		if uuid_value.is_none() {
			// UUID field not present - check if local_field already exists
			if data.get(fk.local_field).is_some() {
				// FK already resolved, skip
				continue;
			} else {
				// Neither field present - set to NULL
				data[fk.local_field] = Value::Null;
				continue;
			}
		}

		// UUID field exists - process it
		if uuid_value.unwrap().is_null() {
			// Null UUID means null FK (e.g., parent_id for root entries)
			data[fk.local_field] = Value::Null;
			// Remove UUID field
			if let Some(obj) = data.as_object_mut() {
				obj.remove(&uuid_field);
			}
			continue;
		}

		let uuid: Uuid = uuid_value
			.and_then(|v| v.as_str())
			.ok_or_else(|| anyhow!("Missing UUID field '{}'", uuid_field))?
			.parse()?;

		// Map UUID to local ID
		// If the referenced record doesn't exist yet (sync dependency), return error for retry
		let local_id = match lookup_local_id_for_uuid(fk.target_table, uuid, db).await {
			Ok(id) => id,
			Err(e) => {
				// Referenced record not found - this is a sync dependency issue
				// Don't set to NULL - instead propagate error so caller can buffer/retry
				return Err(anyhow::anyhow!(
					"Sync dependency missing: {} -> {} (uuid={}): {}",
					fk.local_field,
					fk.target_table,
					uuid,
					e
				));
			}
		};

		// Replace UUID with local ID
		data[fk.local_field] = json!(local_id);

		// Remove UUID field
		if let Some(obj) = data.as_object_mut() {
			obj.remove(&uuid_field);
		}
	}

	Ok(data)
}

/// Batch convert UUIDs to local IDs for multiple records
///
/// This function processes multiple records at once, using batch FK lookups
/// to reduce database queries from N*M (N records × M FKs) to M (one per FK type).

pub async fn batch_map_sync_json_to_local(
	mut records: Vec<Value>,
	mappings: Vec<FKMapping>,
	db: &DatabaseConnection,
) -> Result<Vec<Value>> {
	if records.is_empty() {
		return Ok(records);
	}

	// For each FK mapping, collect all UUIDs from all records, batch lookup, then apply
	for fk in &mappings {
		let uuid_field = fk.uuid_field_name();

		// Collect all UUIDs for this FK type from all records
		let mut uuids_to_lookup: HashSet<Uuid> = HashSet::new();

		for data in &records {
			if let Some(uuid_value) = data.get(&uuid_field) {
				if !uuid_value.is_null() {
					if let Some(uuid_str) = uuid_value.as_str() {
						if let Ok(uuid) = Uuid::parse_str(uuid_str) {
							uuids_to_lookup.insert(uuid);
						}
					}
				}
			}
		}

		// Batch lookup all UUIDs for this FK type (single query)
		let uuid_to_id_map = if !uuids_to_lookup.is_empty() {
			batch_lookup_local_ids_for_uuids(fk.target_table, uuids_to_lookup, db).await?
		} else {
			HashMap::new()
		};

		// Apply mappings to all records using the batch lookup results
		for data in &mut records {
			let uuid_value = data.get(&uuid_field);

			if uuid_value.is_none() || uuid_value.unwrap().is_null() {
				// Null UUID means null FK (e.g., parent_id for root entries)
				data[fk.local_field] = Value::Null;
				continue;
			}

			let uuid: Uuid = match uuid_value
				.and_then(|v| v.as_str())
				.and_then(|s| Uuid::parse_str(s).ok())
			{
				Some(uuid) => uuid,
				None => {
					// Invalid UUID format - set to NULL
					data[fk.local_field] = Value::Null;
					if let Some(obj) = data.as_object_mut() {
						obj.remove(&uuid_field);
					}
					continue;
				}
			};

			// Look up local ID from batch results
			let local_id = match uuid_to_id_map.get(&uuid) {
				Some(&id) => id,
				None => {
					// Referenced record not found - this record has a missing dependency
					// Mark it as failed so it can be filtered out and retried later
					// Setting to NULL would break parent relationships permanently!
					tracing::debug!(
						fk_field = fk.local_field,
						target_table = fk.target_table,
						uuid = %uuid,
						"FK reference not found (dependency missing), marking record for retry"
					);
					// Mark this record as having missing dependency
					data["__fk_mapping_failed"] = json!(true);
					data[fk.local_field] = Value::Null; // Temporary - record will be filtered out
					if let Some(obj) = data.as_object_mut() {
						obj.remove(&uuid_field);
					}
					continue;
				}
			};

			// Replace UUID with local ID
			data[fk.local_field] = json!(local_id);

			// Remove UUID field
			if let Some(obj) = data.as_object_mut() {
				obj.remove(&uuid_field);
			}
		}
	}

	// Filter out records with failed FK mappings (missing dependencies)
	// These will be retried on next sync when dependencies exist
	let original_count = records.len();
	let successful_records: Vec<Value> = records
		.into_iter()
		.filter(|data| {
			let failed = data
				.get("__fk_mapping_failed")
				.and_then(|v| v.as_bool())
				.unwrap_or(false);
			if failed {
				// Clean up marker field
				if let Some(uuid) = data.get("uuid").and_then(|v| v.as_str()) {
					tracing::debug!(
						uuid = uuid,
						"Filtering out record with missing FK dependency (will retry)"
					);
				}
			}
			!failed
		})
		.collect();

	let filtered_count = original_count - successful_records.len();
	if filtered_count > 0 {
		tracing::info!(
			filtered = filtered_count,
			successful = successful_records.len(),
			"Filtered out records with missing FK dependencies (will retry on next sync)"
		);
	}

	Ok(successful_records)
}

/// Look up local integer ID for a UUID in any table
async fn lookup_local_id_for_uuid(table: &str, uuid: Uuid, db: &DatabaseConnection) -> Result<i32> {
	match table {
		"devices" => {
			let device = entities::device::Entity::find()
				.filter(entities::device::Column::Uuid.eq(uuid))
				.one(db)
				.await?
				.ok_or_else(|| {
					anyhow!(
						"Device with uuid={} not found (sync dependency missing)",
						uuid
					)
				})?;
			Ok(device.id)
		}
		"entries" => {
			let entry = entities::entry::Entity::find()
				.filter(entities::entry::Column::Uuid.eq(Some(uuid)))
				.one(db)
				.await?
				.ok_or_else(|| {
					anyhow!(
						"Entry with uuid={} not found (sync dependency missing)",
						uuid
					)
				})?;
			Ok(entry.id)
		}
		"locations" => {
			let location = entities::location::Entity::find()
				.filter(entities::location::Column::Uuid.eq(uuid))
				.one(db)
				.await?
				.ok_or_else(|| anyhow!("Location with uuid={} not found", uuid))?;
			Ok(location.id)
		}
		"user_metadata" => {
			let metadata = entities::user_metadata::Entity::find()
				.filter(entities::user_metadata::Column::Uuid.eq(uuid))
				.one(db)
				.await?
				.ok_or_else(|| {
					anyhow!(
						"UserMetadata with uuid={} not found (sync dependency missing)",
						uuid
					)
				})?;
			Ok(metadata.id)
		}
		"content_identities" => {
			let content = entities::content_identity::Entity::find()
				.filter(entities::content_identity::Column::Uuid.eq(Some(uuid)))
				.one(db)
				.await?
				.ok_or_else(|| {
					anyhow!(
						"ContentIdentity with uuid={} not found (sync dependency missing)",
						uuid
					)
				})?;
			Ok(content.id)
		}
		"volumes" => {
			let volume = entities::volume::Entity::find()
				.filter(entities::volume::Column::Uuid.eq(uuid))
				.one(db)
				.await?
				.ok_or_else(|| {
					anyhow!(
						"Volume with uuid={} not found (sync dependency missing)",
						uuid
					)
				})?;
			Ok(volume.id)
		}
		"collection" => {
			let collection = entities::collection::Entity::find()
				.filter(entities::collection::Column::Uuid.eq(uuid))
				.one(db)
				.await?
				.ok_or_else(|| {
					anyhow!(
						"Collection with uuid={} not found (sync dependency missing)",
						uuid
					)
				})?;
			Ok(collection.id)
		}
		"tag" => {
			let tag = entities::tag::Entity::find()
				.filter(entities::tag::Column::Uuid.eq(uuid))
				.one(db)
				.await?
				.ok_or_else(|| {
					anyhow!("Tag with uuid={} not found (sync dependency missing)", uuid)
				})?;
			Ok(tag.id)
		}
		_ => Err(anyhow!("Unknown table for FK mapping: {}", table)),
	}
}

/// Batch look up local integer IDs for multiple UUIDs in any table
///
/// Returns a HashMap mapping UUID -> local_id for all found records.
/// Records not found are omitted from the result map (caller must handle missing entries).
///
/// This function performs a single SQL query with WHERE uuid IN (...) instead of
/// N individual queries, significantly reducing database load during sync.
pub async fn batch_lookup_local_ids_for_uuids(
	table: &str,
	uuids: HashSet<Uuid>,
	db: &DatabaseConnection,
) -> Result<HashMap<Uuid, i32>> {
	if uuids.is_empty() {
		return Ok(HashMap::new());
	}

	let uuid_vec: Vec<Uuid> = uuids.into_iter().collect();

	match table {
		"devices" => {
			let records = entities::device::Entity::find()
				.filter(entities::device::Column::Uuid.is_in(uuid_vec))
				.all(db)
				.await?;
			Ok(records.into_iter().map(|r| (r.uuid, r.id)).collect())
		}
		"entries" => {
			let records = entities::entry::Entity::find()
				.filter(entities::entry::Column::Uuid.is_in(uuid_vec))
				.all(db)
				.await?;
			let mut map = HashMap::new();
			for r in records {
				if let Some(uuid) = r.uuid {
					map.insert(uuid, r.id);
				}
			}
			Ok(map)
		}
		"locations" => {
			let records = entities::location::Entity::find()
				.filter(entities::location::Column::Uuid.is_in(uuid_vec))
				.all(db)
				.await?;
			Ok(records.into_iter().map(|r| (r.uuid, r.id)).collect())
		}
		"user_metadata" => {
			let records = entities::user_metadata::Entity::find()
				.filter(entities::user_metadata::Column::Uuid.is_in(uuid_vec))
				.all(db)
				.await?;
			Ok(records.into_iter().map(|r| (r.uuid, r.id)).collect())
		}
		"content_identities" => {
			let records = entities::content_identity::Entity::find()
				.filter(entities::content_identity::Column::Uuid.is_in(uuid_vec))
				.all(db)
				.await?;
			let mut map = HashMap::new();
			for r in records {
				if let Some(uuid) = r.uuid {
					map.insert(uuid, r.id);
				}
			}
			Ok(map)
		}
		"volumes" => {
			let records = entities::volume::Entity::find()
				.filter(entities::volume::Column::Uuid.is_in(uuid_vec))
				.all(db)
				.await?;
			Ok(records.into_iter().map(|r| (r.uuid, r.id)).collect())
		}
		"collection" => {
			let records = entities::collection::Entity::find()
				.filter(entities::collection::Column::Uuid.is_in(uuid_vec))
				.all(db)
				.await?;
			Ok(records.into_iter().map(|r| (r.uuid, r.id)).collect())
		}
		"tag" => {
			let records = entities::tag::Entity::find()
				.filter(entities::tag::Column::Uuid.is_in(uuid_vec))
				.all(db)
				.await?;
			Ok(records.into_iter().map(|r| (r.uuid, r.id)).collect())
		}
		_ => Err(anyhow!("Unknown table for FK mapping: {}", table)),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_uuid_field_name() {
		let fk = FKMapping::new("device_id", "devices");
		assert_eq!(fk.uuid_field_name(), "device_uuid");

		let fk = FKMapping::new("parent_id", "entries");
		assert_eq!(fk.uuid_field_name(), "parent_uuid");

		let fk = FKMapping::new("entry_id", "entries");
		assert_eq!(fk.uuid_field_name(), "entry_uuid");
	}
}
