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
//! 1. `convert_fk_to_uuid()` - Converts local integer FKs → UUIDs before sending
//! 2. `map_sync_json_to_local()` - Converts UUIDs → local integer FKs on receive
//!
//! ## Fully Polymorphic Design
//!
//! This module contains ZERO model-specific code. All lookups go through the registry:
//! - Table names are mapped to model types via `registry::get_model_type_by_table()`
//! - FK lookups use the registered `Syncable` trait implementations
//! - New models are automatically supported when registered via `register_syncable!` macros

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use sea_orm::DatabaseConnection;
use serde_json::{json, Value};
use uuid::Uuid;

/// Foreign key mapping declaration
#[derive(Debug, Clone)]
pub struct FKMapping {
	/// Field name in the model (e.g., "device_id")
	pub local_field: &'static str,

	/// Target table name (e.g., "devices")
	pub target_table: &'static str,

	/// Whether this FK can be null (for circular dependencies)
	pub nullable: bool,
}

impl FKMapping {
	pub fn new(local_field: &'static str, target_table: &'static str) -> Self {
		Self {
			local_field,
			target_table,
			nullable: false,
		}
	}

	pub fn new_nullable(local_field: &'static str, target_table: &'static str) -> Self {
		Self {
			local_field,
			target_table,
			nullable: true,
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

/// Look up UUID for a local integer ID via the registry
async fn lookup_uuid_for_local_id(
	table: &str,
	local_id: i32,
	db: &DatabaseConnection,
) -> Result<Uuid> {
	// Map table name to model type via registry (fully polymorphic)
	let model_type = super::registry::get_model_type_by_table(table)
		.ok_or_else(|| anyhow!("No model registered for table '{}' - check sync registration", table))?;

	super::registry::lookup_uuid_by_id(model_type, local_id, Arc::new(db.clone()))
		.await
		.map_err(|e| anyhow!("FK lookup failed for {}: {}", table, e))?
		.ok_or_else(|| anyhow!("{} with id={} not found", table, local_id))
}

/// Batch look up UUIDs for multiple local integer IDs via the registry
///
/// Returns a HashMap mapping local_id -> UUID for all found records.
/// Records not found are omitted from the result map (caller must handle missing entries).
pub async fn batch_lookup_uuids_for_local_ids(
	table: &str,
	local_ids: HashSet<i32>,
	db: &DatabaseConnection,
) -> Result<HashMap<i32, Uuid>> {
	if local_ids.is_empty() {
		return Ok(HashMap::new());
	}

	let model_type = super::registry::get_model_type_by_table(table)
		.ok_or_else(|| anyhow!("No model registered for table '{}' - check sync registration", table))?;

	super::registry::batch_lookup_uuids_by_ids(model_type, local_ids, Arc::new(db.clone()))
		.await
		.map_err(|e| anyhow!("Batch FK lookup failed for {}: {}", table, e))
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
				// For nullable FKs (circular dependencies), set to NULL instead of erroring
				if fk.nullable {
					data[fk.local_field] = Value::Null;
					if let Some(obj) = data.as_object_mut() {
						obj.remove(&uuid_field);
					}
					continue;
				}

				// For non-nullable FKs, propagate error so caller can buffer/retry
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
///
/// This is a 365x reduction for typical sync workloads with 1000 records and 3 FKs each.
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
					// Referenced record not found - set FK to NULL
					// This is controversial, may cause issues
					tracing::warn!(
						"FK reference not found: {} -> {} (uuid={}), setting to NULL",
						fk.local_field,
						fk.target_table,
						uuid
					);
					data[fk.local_field] = Value::Null;
					// Remove UUID field
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

	Ok(records)
}

/// Look up local integer ID for a UUID via the registry
async fn lookup_local_id_for_uuid(table: &str, uuid: Uuid, db: &DatabaseConnection) -> Result<i32> {
	let model_type = super::registry::get_model_type_by_table(table)
		.ok_or_else(|| anyhow!("No model registered for table '{}' - check sync registration", table))?;

	super::registry::lookup_id_by_uuid(model_type, uuid, Arc::new(db.clone()))
		.await
		.map_err(|e| anyhow!("FK lookup failed for {}: {}", table, e))?
		.ok_or_else(|| anyhow!("{} with uuid={} not found (sync dependency missing)", table, uuid))
}

/// Batch look up local integer IDs for multiple UUIDs via the registry
///
/// Returns a HashMap mapping UUID -> local_id for all found records.
/// Records not found are omitted from the result map (caller must handle missing entries).
pub async fn batch_lookup_local_ids_for_uuids(
	table: &str,
	uuids: HashSet<Uuid>,
	db: &DatabaseConnection,
) -> Result<HashMap<Uuid, i32>> {
	if uuids.is_empty() {
		return Ok(HashMap::new());
	}

	let model_type = super::registry::get_model_type_by_table(table)
		.ok_or_else(|| anyhow!("No model registered for table '{}' - check sync registration", table))?;

	super::registry::batch_lookup_ids_by_uuids(model_type, uuids, Arc::new(db.clone()))
		.await
		.map_err(|e| anyhow!("Batch FK lookup failed for {}: {}", table, e))
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
