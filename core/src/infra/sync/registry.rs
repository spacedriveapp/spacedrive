//! Syncable model registry
//!
//! Provides a runtime registry of all syncable models for dynamic dispatch.
//! This enables the sync applier to deserialize and apply changes without
//! knowing the concrete model type at compile time.
//!
//! Models self-register using the `register_syncable!` macro, which uses
//! the `inventory` crate to collect registrations at link time.

use super::{SharedChangeEntry, Syncable};
use sea_orm::DatabaseConnection;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::RwLock;

use once_cell::sync::Lazy;

// =============================================================================
// Inventory-based Registration
// =============================================================================

/// Entry submitted to inventory for syncable model registration.
/// Models submit these via the `register_syncable!` macro.
pub struct SyncableInventoryEntry {
	/// Function that builds the full registration
	pub build: fn() -> SyncableModelRegistration,
}

// Tell inventory about our entry type
inventory::collect!(SyncableInventoryEntry);

/// Register a device-owned syncable model.
///
/// Usage in entity file:
/// ```ignore
/// register_syncable_device_owned!(Model, "location", "locations");
/// // With deletion support:
/// register_syncable_device_owned!(Model, "location", "locations", with_deletion);
/// // With post-backfill rebuild:
/// register_syncable_device_owned!(Model, "entry", "entries", with_deletion, with_rebuild);
/// ```
#[macro_export]
macro_rules! register_syncable_device_owned {
	// Base case: no deletion, no rebuild
	($model:ty, $model_name:literal, $table_name:literal) => {
		inventory::submit! {
			$crate::infra::sync::SyncableInventoryEntry {
				build: || {
					$crate::infra::sync::SyncableModelRegistration::device_owned(
						$model_name,
						$table_name,
						|data, db| Box::pin(async move {
							<$model as $crate::infra::sync::Syncable>::apply_state_change(data, db.as_ref()).await
						}),
						|device_id, since, cursor, batch_size, db| Box::pin(async move {
							<$model as $crate::infra::sync::Syncable>::query_for_sync(device_id, since, cursor, batch_size, db.as_ref()).await
						}),
						None,
					)
					.with_fk_lookups(
						|uuid, db| Box::pin(async move { <$model as $crate::infra::sync::Syncable>::lookup_id_by_uuid(uuid, db.as_ref()).await }),
						|id, db| Box::pin(async move { <$model as $crate::infra::sync::Syncable>::lookup_uuid_by_id(id, db.as_ref()).await }),
						|uuids, db| Box::pin(async move { <$model as $crate::infra::sync::Syncable>::batch_lookup_ids_by_uuids(uuids, db.as_ref()).await }),
						|ids, db| Box::pin(async move { <$model as $crate::infra::sync::Syncable>::batch_lookup_uuids_by_ids(ids, db.as_ref()).await }),
					)
					.with_fk_mappings(<$model as $crate::infra::sync::Syncable>::foreign_key_mappings)
					.with_depends_on(<$model as $crate::infra::sync::Syncable>::sync_depends_on)
				}
			}
		}
	};

	// With deletion support
	($model:ty, $model_name:literal, $table_name:literal, with_deletion) => {
		inventory::submit! {
			$crate::infra::sync::SyncableInventoryEntry {
				build: || {
					$crate::infra::sync::SyncableModelRegistration::device_owned(
						$model_name,
						$table_name,
						|data, db| Box::pin(async move {
							<$model as $crate::infra::sync::Syncable>::apply_state_change(data, db.as_ref()).await
						}),
						|device_id, since, cursor, batch_size, db| Box::pin(async move {
							<$model as $crate::infra::sync::Syncable>::query_for_sync(device_id, since, cursor, batch_size, db.as_ref()).await
						}),
						Some(|uuid, db| Box::pin(async move {
							<$model as $crate::infra::sync::Syncable>::apply_deletion(uuid, db.as_ref()).await
						})),
					)
					.with_fk_lookups(
						|uuid, db| Box::pin(async move { <$model as $crate::infra::sync::Syncable>::lookup_id_by_uuid(uuid, db.as_ref()).await }),
						|id, db| Box::pin(async move { <$model as $crate::infra::sync::Syncable>::lookup_uuid_by_id(id, db.as_ref()).await }),
						|uuids, db| Box::pin(async move { <$model as $crate::infra::sync::Syncable>::batch_lookup_ids_by_uuids(uuids, db.as_ref()).await }),
						|ids, db| Box::pin(async move { <$model as $crate::infra::sync::Syncable>::batch_lookup_uuids_by_ids(ids, db.as_ref()).await }),
					)
					.with_fk_mappings(<$model as $crate::infra::sync::Syncable>::foreign_key_mappings)
					.with_depends_on(<$model as $crate::infra::sync::Syncable>::sync_depends_on)
				}
			}
		}
	};

	// With deletion and post-backfill rebuild
	($model:ty, $model_name:literal, $table_name:literal, with_deletion, with_rebuild) => {
		inventory::submit! {
			$crate::infra::sync::SyncableInventoryEntry {
				build: || {
					$crate::infra::sync::SyncableModelRegistration::device_owned(
						$model_name,
						$table_name,
						|data, db| Box::pin(async move {
							<$model as $crate::infra::sync::Syncable>::apply_state_change(data, db.as_ref()).await
						}),
						|device_id, since, cursor, batch_size, db| Box::pin(async move {
							<$model as $crate::infra::sync::Syncable>::query_for_sync(device_id, since, cursor, batch_size, db.as_ref()).await
						}),
						Some(|uuid, db| Box::pin(async move {
							<$model as $crate::infra::sync::Syncable>::apply_deletion(uuid, db.as_ref()).await
						})),
					)
					.with_fk_lookups(
						|uuid, db| Box::pin(async move { <$model as $crate::infra::sync::Syncable>::lookup_id_by_uuid(uuid, db.as_ref()).await }),
						|id, db| Box::pin(async move { <$model as $crate::infra::sync::Syncable>::lookup_uuid_by_id(id, db.as_ref()).await }),
						|uuids, db| Box::pin(async move { <$model as $crate::infra::sync::Syncable>::batch_lookup_ids_by_uuids(uuids, db.as_ref()).await }),
						|ids, db| Box::pin(async move { <$model as $crate::infra::sync::Syncable>::batch_lookup_uuids_by_ids(ids, db.as_ref()).await }),
					)
					.with_fk_mappings(<$model as $crate::infra::sync::Syncable>::foreign_key_mappings)
					.with_depends_on(<$model as $crate::infra::sync::Syncable>::sync_depends_on)
					.with_post_backfill_rebuild(|db| Box::pin(async move {
						<$model as $crate::infra::sync::Syncable>::post_backfill_rebuild(db.as_ref()).await
					}))
				}
			}
		}
	};
}

/// Register a shared syncable model (log-based sync).
///
/// Usage in entity file:
/// ```ignore
/// register_syncable_shared!(Model, "tag", "tag");
/// ```
#[macro_export]
macro_rules! register_syncable_shared {
	($model:ty, $model_name:literal, $table_name:literal) => {
		inventory::submit! {
			$crate::infra::sync::SyncableInventoryEntry {
				build: || {
					$crate::infra::sync::SyncableModelRegistration::shared_with_query(
						$model_name,
						$table_name,
						|entry, db| Box::pin(async move {
							<$model as $crate::infra::sync::Syncable>::apply_shared_change(entry, db.as_ref()).await
						}),
						|device_id, since, cursor, batch_size, db| Box::pin(async move {
							<$model as $crate::infra::sync::Syncable>::query_for_sync(device_id, since, cursor, batch_size, db.as_ref()).await
						}),
					)
					.with_fk_lookups(
						|uuid, db| Box::pin(async move { <$model as $crate::infra::sync::Syncable>::lookup_id_by_uuid(uuid, db.as_ref()).await }),
						|id, db| Box::pin(async move { <$model as $crate::infra::sync::Syncable>::lookup_uuid_by_id(id, db.as_ref()).await }),
						|uuids, db| Box::pin(async move { <$model as $crate::infra::sync::Syncable>::batch_lookup_ids_by_uuids(uuids, db.as_ref()).await }),
						|ids, db| Box::pin(async move { <$model as $crate::infra::sync::Syncable>::batch_lookup_uuids_by_ids(ids, db.as_ref()).await }),
					)
					.with_fk_mappings(<$model as $crate::infra::sync::Syncable>::foreign_key_mappings)
					.with_depends_on(<$model as $crate::infra::sync::Syncable>::sync_depends_on)
				}
			}
		}
	};
	// Variant with post-backfill rebuild support
	($model:ty, $model_name:literal, $table_name:literal, with_rebuild) => {
		inventory::submit! {
			$crate::infra::sync::SyncableInventoryEntry {
				build: || {
					$crate::infra::sync::SyncableModelRegistration::shared_with_query(
						$model_name,
						$table_name,
						|entry, db| Box::pin(async move {
							<$model as $crate::infra::sync::Syncable>::apply_shared_change(entry, db.as_ref()).await
						}),
						|device_id, since, cursor, batch_size, db| Box::pin(async move {
							<$model as $crate::infra::sync::Syncable>::query_for_sync(device_id, since, cursor, batch_size, db.as_ref()).await
						}),
					)
					.with_fk_lookups(
						|uuid, db| Box::pin(async move { <$model as $crate::infra::sync::Syncable>::lookup_id_by_uuid(uuid, db.as_ref()).await }),
						|id, db| Box::pin(async move { <$model as $crate::infra::sync::Syncable>::lookup_uuid_by_id(id, db.as_ref()).await }),
						|uuids, db| Box::pin(async move { <$model as $crate::infra::sync::Syncable>::batch_lookup_ids_by_uuids(uuids, db.as_ref()).await }),
						|ids, db| Box::pin(async move { <$model as $crate::infra::sync::Syncable>::batch_lookup_uuids_by_ids(ids, db.as_ref()).await }),
					)
					.with_fk_mappings(<$model as $crate::infra::sync::Syncable>::foreign_key_mappings)
					.with_depends_on(<$model as $crate::infra::sync::Syncable>::sync_depends_on)
					.with_post_backfill_rebuild(|db| Box::pin(async move {
						<$model as $crate::infra::sync::Syncable>::post_backfill_rebuild(db.as_ref()).await
					}))
				}
			}
		}
	};
}

/// Type alias for state-based apply function (device-owned models)
pub type StateApplyFn = fn(
	serde_json::Value,
	Arc<DatabaseConnection>,
) -> Pin<Box<dyn Future<Output = Result<(), sea_orm::DbErr>> + Send>>;

/// Type alias for log-based apply function (shared models)
pub type SharedApplyFn = fn(
	SharedChangeEntry,
	Arc<DatabaseConnection>,
) -> Pin<Box<dyn Future<Output = Result<(), sea_orm::DbErr>> + Send>>;

/// Type alias for state query function (device-owned models)
///
/// Parameters: device_id, since, batch_size, db
/// Returns: Vec of (uuid, data, timestamp)
pub type StateQueryFn = fn(
	Option<uuid::Uuid>,                                  // device_id filter
	Option<chrono::DateTime<chrono::Utc>>,               // since watermark
	Option<(chrono::DateTime<chrono::Utc>, uuid::Uuid)>, // cursor for pagination
	usize,                                               // batch_size
	Arc<DatabaseConnection>,
) -> Pin<
	Box<
		dyn Future<
				Output = Result<
					Vec<(uuid::Uuid, serde_json::Value, chrono::DateTime<chrono::Utc>)>,
					sea_orm::DbErr,
				>,
			> + Send,
	>,
>;

/// Type alias for deletion apply function (device-owned models)
///
/// Parameters: uuid of record to delete, db
/// Returns: Result indicating success/failure
pub type StateDeleteFn = fn(
	uuid::Uuid,
	Arc<DatabaseConnection>,
) -> Pin<Box<dyn Future<Output = Result<(), sea_orm::DbErr>> + Send>>;

/// Type alias for FK ID lookup by UUID
pub type FkLookupIdFn =
	fn(
		uuid::Uuid,
		Arc<DatabaseConnection>,
	) -> Pin<Box<dyn Future<Output = Result<Option<i32>, sea_orm::DbErr>> + Send>>;

/// Type alias for FK UUID lookup by ID
pub type FkLookupUuidFn =
	fn(
		i32,
		Arc<DatabaseConnection>,
	) -> Pin<Box<dyn Future<Output = Result<Option<uuid::Uuid>, sea_orm::DbErr>> + Send>>;

/// Type alias for batch FK ID lookup by UUIDs
pub type FkBatchLookupIdsFn = fn(
	std::collections::HashSet<uuid::Uuid>,
	Arc<DatabaseConnection>,
) -> Pin<
	Box<
		dyn Future<Output = Result<std::collections::HashMap<uuid::Uuid, i32>, sea_orm::DbErr>>
			+ Send,
	>,
>;

/// Type alias for batch FK UUID lookup by IDs
pub type FkBatchLookupUuidsFn = fn(
	std::collections::HashSet<i32>,
	Arc<DatabaseConnection>,
) -> Pin<
	Box<
		dyn Future<Output = Result<std::collections::HashMap<i32, uuid::Uuid>, sea_orm::DbErr>>
			+ Send,
	>,
>;

/// Type alias for post-backfill rebuild function
pub type PostBackfillRebuildFn =
	fn(Arc<DatabaseConnection>) -> Pin<Box<dyn Future<Output = Result<(), sea_orm::DbErr>> + Send>>;

/// Type alias for FK mappings function
pub type FkMappingsFn = fn() -> Vec<super::FKMapping>;

/// Type alias for sync depends on function
pub type SyncDependsOnFn = fn() -> &'static [&'static str];

/// Registry of syncable models
///
/// Maps model_type strings (e.g., "album", "tag") to their registration info.
pub static SYNCABLE_REGISTRY: Lazy<RwLock<HashMap<String, SyncableModelRegistration>>> =
	Lazy::new(|| {
		// Initialize registry with all models
		RwLock::new(initialize_registry())
	});

/// Registration information for a syncable model
pub struct SyncableModelRegistration {
	/// Model type identifier (e.g., "location")
	pub model_type: &'static str,

	/// Table name in database (e.g., "locations")
	pub table_name: &'static str,

	/// Whether this is device-owned (state-based) or shared (log-based)
	pub is_device_owned: bool,

	/// Apply function for state-based sync (device-owned models)
	pub state_apply_fn: Option<StateApplyFn>,

	/// Apply function for log-based sync (shared models)
	pub shared_apply_fn: Option<SharedApplyFn>,

	/// Query function for backfill (both device-owned and shared models)
	pub state_query_fn: Option<StateQueryFn>,

	/// Deletion apply function for device-owned models
	pub state_delete_fn: Option<StateDeleteFn>,

	// FK lookup functions (for models used as FK targets)
	/// Lookup local ID by UUID
	pub fk_lookup_id_fn: Option<FkLookupIdFn>,
	/// Lookup UUID by local ID
	pub fk_lookup_uuid_fn: Option<FkLookupUuidFn>,
	/// Batch lookup local IDs by UUIDs
	pub fk_batch_lookup_ids_fn: Option<FkBatchLookupIdsFn>,
	/// Batch lookup UUIDs by local IDs
	pub fk_batch_lookup_uuids_fn: Option<FkBatchLookupUuidsFn>,

	// Trait metadata functions
	/// Get FK mappings for this model
	pub fk_mappings_fn: Option<FkMappingsFn>,
	/// Get sync dependencies for this model
	pub sync_depends_on_fn: Option<SyncDependsOnFn>,
	/// Post-backfill rebuild function (e.g., for closure tables)
	pub post_backfill_rebuild_fn: Option<PostBackfillRebuildFn>,
}

impl SyncableModelRegistration {
	/// Create a new registration for device-owned model
	pub fn device_owned(
		model_type: &'static str,
		table_name: &'static str,
		apply_fn: StateApplyFn,
		query_fn: StateQueryFn,
		delete_fn: Option<StateDeleteFn>,
	) -> Self {
		Self {
			model_type,
			table_name,
			is_device_owned: true,
			state_apply_fn: Some(apply_fn),
			shared_apply_fn: None,
			state_query_fn: Some(query_fn),
			state_delete_fn: delete_fn,
			fk_lookup_id_fn: None,
			fk_lookup_uuid_fn: None,
			fk_batch_lookup_ids_fn: None,
			fk_batch_lookup_uuids_fn: None,
			fk_mappings_fn: None,
			sync_depends_on_fn: None,
			post_backfill_rebuild_fn: None,
		}
	}

	/// Create a new registration for shared model
	pub fn shared(
		model_type: &'static str,
		table_name: &'static str,
		apply_fn: SharedApplyFn,
	) -> Self {
		Self {
			model_type,
			table_name,
			is_device_owned: false,
			state_apply_fn: None,
			shared_apply_fn: Some(apply_fn),
			state_query_fn: None,
			state_delete_fn: None,
			fk_lookup_id_fn: None,
			fk_lookup_uuid_fn: None,
			fk_batch_lookup_ids_fn: None,
			fk_batch_lookup_uuids_fn: None,
			fk_mappings_fn: None,
			sync_depends_on_fn: None,
			post_backfill_rebuild_fn: None,
		}
	}

	/// Create a new registration for shared model with query function (for backfill)
	pub fn shared_with_query(
		model_type: &'static str,
		table_name: &'static str,
		apply_fn: SharedApplyFn,
		query_fn: StateQueryFn,
	) -> Self {
		Self {
			model_type,
			table_name,
			is_device_owned: false,
			state_apply_fn: None,
			shared_apply_fn: Some(apply_fn),
			state_query_fn: Some(query_fn),
			state_delete_fn: None,
			fk_lookup_id_fn: None,
			fk_lookup_uuid_fn: None,
			fk_batch_lookup_ids_fn: None,
			fk_batch_lookup_uuids_fn: None,
			fk_mappings_fn: None,
			sync_depends_on_fn: None,
			post_backfill_rebuild_fn: None,
		}
	}

	/// Builder method to add FK lookup functions
	pub fn with_fk_lookups(
		mut self,
		lookup_id: FkLookupIdFn,
		lookup_uuid: FkLookupUuidFn,
		batch_lookup_ids: FkBatchLookupIdsFn,
		batch_lookup_uuids: FkBatchLookupUuidsFn,
	) -> Self {
		self.fk_lookup_id_fn = Some(lookup_id);
		self.fk_lookup_uuid_fn = Some(lookup_uuid);
		self.fk_batch_lookup_ids_fn = Some(batch_lookup_ids);
		self.fk_batch_lookup_uuids_fn = Some(batch_lookup_uuids);
		self
	}

	/// Builder method to add FK mappings function
	pub fn with_fk_mappings(mut self, fk_mappings: FkMappingsFn) -> Self {
		self.fk_mappings_fn = Some(fk_mappings);
		self
	}

	/// Builder method to add sync dependencies function
	pub fn with_depends_on(mut self, depends_on: SyncDependsOnFn) -> Self {
		self.sync_depends_on_fn = Some(depends_on);
		self
	}

	/// Builder method to add post-backfill rebuild function
	pub fn with_post_backfill_rebuild(mut self, rebuild_fn: PostBackfillRebuildFn) -> Self {
		self.post_backfill_rebuild_fn = Some(rebuild_fn);
		self
	}
}

/// Register a device-owned model with state-based apply and query functions
pub async fn register_device_owned(
	model_type: &'static str,
	table_name: &'static str,
	apply_fn: StateApplyFn,
	query_fn: StateQueryFn,
	delete_fn: Option<StateDeleteFn>,
) {
	let mut registry = SYNCABLE_REGISTRY.write().await;
	registry.insert(
		model_type.to_string(),
		SyncableModelRegistration::device_owned(
			model_type, table_name, apply_fn, query_fn, delete_fn,
		),
	);
}

/// Register a shared model with log-based apply function
pub async fn register_shared(
	model_type: &'static str,
	table_name: &'static str,
	apply_fn: SharedApplyFn,
) {
	let mut registry = SYNCABLE_REGISTRY.write().await;
	registry.insert(
		model_type.to_string(),
		SyncableModelRegistration::shared(model_type, table_name, apply_fn),
	);
}

/// Initialize registry from inventory submissions
///
/// Models self-register via the `register_syncable!` macro.
/// This function collects all submissions at runtime.
fn initialize_registry() -> HashMap<String, SyncableModelRegistration> {
	let mut registry = HashMap::new();

	for entry in inventory::iter::<SyncableInventoryEntry> {
		let registration = (entry.build)();
		registry.insert(registration.model_type.to_string(), registration);
	}

	registry
}

/// Get table name for a model type
pub async fn get_table_name(model_type: &str) -> Option<&'static str> {
	SYNCABLE_REGISTRY
		.read()
		.await
		.get(model_type)
		.map(|reg| reg.table_name)
}

/// Get model type for a table name (reverse lookup)
///
/// This is the inverse of `get_table_name()`. Used by FK mapper to convert
/// table names (from FKMapping declarations) to model types (for registry lookup).
pub fn get_model_type_by_table(table: &str) -> Option<&'static str> {
	if let Ok(registry) = SYNCABLE_REGISTRY.try_read() {
		for reg in registry.values() {
			if reg.table_name == table {
				return Some(reg.model_type);
			}
		}
	}
	None
}

/// Check if model is device-owned
pub async fn is_device_owned(model_type: &str) -> bool {
	SYNCABLE_REGISTRY
		.read()
		.await
		.get(model_type)
		.map(|reg| reg.is_device_owned)
		.unwrap_or(false)
}

/// Get foreign key mappings for a model type
///
/// Returns the FK mappings defined by each model's Syncable implementation.
/// Used for batch FK resolution during sync to reduce database queries from N*M to M.
pub fn get_fk_mappings(model_type: &str) -> Option<Vec<super::FKMapping>> {
	if let Ok(registry) = SYNCABLE_REGISTRY.try_read() {
		if let Some(reg) = registry.get(model_type) {
			if let Some(fk_fn) = reg.fk_mappings_fn {
				return Some(fk_fn());
			}
		}
	}
	None
}

/// Apply a state-based sync entry (device-owned model)
///
/// Routes to the appropriate model's apply_state_change function via registry.
pub async fn apply_state_change(
	model_type: &str,
	data: serde_json::Value,
	db: Arc<DatabaseConnection>,
) -> Result<(), ApplyError> {
	let apply_fn = {
		let registry = SYNCABLE_REGISTRY.read().await;
		let registration = registry
			.get(model_type)
			.ok_or_else(|| ApplyError::UnknownModel(model_type.to_string()))?;

		if !registration.is_device_owned {
			return Err(ApplyError::WrongSyncType {
				model: model_type.to_string(),
				expected: "device-owned".to_string(),
				got: "shared".to_string(),
			});
		}

		registration
			.state_apply_fn
			.ok_or_else(|| ApplyError::MissingApplyFunction(model_type.to_string()))?
	}; // Lock is dropped here

	// Call the registered apply function
	apply_fn(data, db)
		.await
		.map_err(|e| ApplyError::DatabaseError(e.to_string()))
}

/// Apply a deletion by UUID (device-owned model)
///
/// Routes to the appropriate model's apply_deletion function via registry.
pub async fn apply_deletion(
	model_type: &str,
	uuid: uuid::Uuid,
	db: Arc<DatabaseConnection>,
) -> Result<(), ApplyError> {
	let delete_fn = {
		let registry = SYNCABLE_REGISTRY.read().await;
		let registration = registry
			.get(model_type)
			.ok_or_else(|| ApplyError::UnknownModel(model_type.to_string()))?;

		if !registration.is_device_owned {
			return Err(ApplyError::WrongSyncType {
				model: model_type.to_string(),
				expected: "device-owned".to_string(),
				got: "shared".to_string(),
			});
		}

		registration
			.state_delete_fn
			.ok_or_else(|| ApplyError::MissingDeletionHandler(model_type.to_string()))?
	}; // Lock is dropped here

	// Call the registered deletion function
	delete_fn(uuid, db)
		.await
		.map_err(|e| ApplyError::DatabaseError(e.to_string()))
}

/// Apply a log-based sync entry (shared model)
///
/// Routes to the appropriate model's apply_shared_change function via registry.
pub async fn apply_shared_change(
	entry: SharedChangeEntry,
	db: Arc<DatabaseConnection>,
) -> Result<(), ApplyError> {
	let apply_fn = {
		let registry = SYNCABLE_REGISTRY.read().await;
		let registration = registry
			.get(&entry.model_type)
			.ok_or_else(|| ApplyError::UnknownModel(entry.model_type.clone()))?;

		if registration.is_device_owned {
			return Err(ApplyError::WrongSyncType {
				model: entry.model_type.clone(),
				expected: "shared".to_string(),
				got: "device-owned".to_string(),
			});
		}

		registration
			.shared_apply_fn
			.ok_or_else(|| ApplyError::MissingApplyFunction(entry.model_type.clone()))?
	}; // Lock is dropped here

	// Call the registered apply function
	apply_fn(entry, db)
		.await
		.map_err(|e| ApplyError::DatabaseError(e.to_string()))
}

/// Query device state for a model type (for backfill)
///
/// Routes to the appropriate model's query function via registry.
pub async fn query_device_state(
	model_type: &str,
	device_id: Option<uuid::Uuid>,
	since: Option<chrono::DateTime<chrono::Utc>>,
	cursor: Option<(chrono::DateTime<chrono::Utc>, uuid::Uuid)>,
	batch_size: usize,
	db: Arc<DatabaseConnection>,
) -> Result<Vec<(uuid::Uuid, serde_json::Value, chrono::DateTime<chrono::Utc>)>, ApplyError> {
	let query_fn = {
		let registry = SYNCABLE_REGISTRY.read().await;
		let registration = registry
			.get(model_type)
			.ok_or_else(|| ApplyError::UnknownModel(model_type.to_string()))?;

		if !registration.is_device_owned {
			return Err(ApplyError::WrongSyncType {
				model: model_type.to_string(),
				expected: "device-owned".to_string(),
				got: "shared".to_string(),
			});
		}

		registration
			.state_query_fn
			.ok_or_else(|| ApplyError::MissingQueryFunction(model_type.to_string()))?
	}; // Lock is dropped here

	// Call the registered query function with cursor for pagination
	query_fn(device_id, since, cursor, batch_size, db)
		.await
		.map_err(|e| ApplyError::DatabaseError(e.to_string()))
}

/// Query all shared models for backfill (generic registry-based approach)
///
/// This discovers and queries ALL shared models registered in the system,
/// making it completely generic - no need to modify this when adding new shared models.
///
/// # Parameters
/// - `since`: Optional timestamp filter
/// - `batch_size`: Max records per model type
/// - `db`: Database connection
///
/// # Returns
/// HashMap mapping model_type -> Vec of (uuid, data, timestamp) tuples
pub async fn query_all_shared_models(
	since: Option<chrono::DateTime<chrono::Utc>>,
	batch_size: usize,
	db: Arc<DatabaseConnection>,
) -> Result<
	HashMap<String, Vec<(uuid::Uuid, serde_json::Value, chrono::DateTime<chrono::Utc>)>>,
	ApplyError,
> {
	// Collect all shared models with query functions
	let shared_models: Vec<(String, StateQueryFn)> = {
		let registry = SYNCABLE_REGISTRY.read().await;
		registry
			.iter()
			.filter(|(_, reg)| !reg.is_device_owned && reg.state_query_fn.is_some())
			.map(|(model_type, reg)| (model_type.clone(), reg.state_query_fn.unwrap()))
			.collect()
	}; // Lock dropped

	// Query all models concurrently
	let mut results = HashMap::new();

	for (model_type, query_fn) in shared_models {
		match query_fn(None, since, None, batch_size, db.clone()).await {
			Ok(records) => {
				if !records.is_empty() {
					tracing::info!(
						model_type = %model_type,
						count = records.len(),
						"Queried shared model for backfill"
					);
					results.insert(model_type, records);
				}
			}
			Err(e) => {
				// Log error but continue - table might not exist yet (e.g., in tests)
				tracing::warn!(
					model_type = %model_type,
					error = %e,
					"Failed to query shared model, skipping (table may not exist)"
				);
			}
		}
	}

	Ok(results)
}

// =============================================================================
// FK Lookup Functions (for generic FK mapping)
// =============================================================================

/// Lookup local ID by UUID via the registry
///
/// This is the generic replacement for the match statements in fk_mapper.rs.
/// Returns an error if the model type is not registered or doesn't have FK lookups.
pub async fn lookup_id_by_uuid(
	table: &str,
	uuid: uuid::Uuid,
	db: Arc<DatabaseConnection>,
) -> Result<Option<i32>, ApplyError> {
	let lookup_fn = {
		let registry = SYNCABLE_REGISTRY.read().await;
		let reg = registry
			.get(table)
			.ok_or_else(|| ApplyError::UnknownModel(table.to_string()))?;
		reg.fk_lookup_id_fn
			.ok_or_else(|| ApplyError::MissingFkLookup(table.to_string()))?
	};

	lookup_fn(uuid, db)
		.await
		.map_err(|e| ApplyError::DatabaseError(e.to_string()))
}

/// Lookup UUID by local ID via the registry
pub async fn lookup_uuid_by_id(
	table: &str,
	id: i32,
	db: Arc<DatabaseConnection>,
) -> Result<Option<uuid::Uuid>, ApplyError> {
	let lookup_fn = {
		let registry = SYNCABLE_REGISTRY.read().await;
		let reg = registry
			.get(table)
			.ok_or_else(|| ApplyError::UnknownModel(table.to_string()))?;
		reg.fk_lookup_uuid_fn
			.ok_or_else(|| ApplyError::MissingFkLookup(table.to_string()))?
	};

	lookup_fn(id, db)
		.await
		.map_err(|e| ApplyError::DatabaseError(e.to_string()))
}

/// Batch lookup local IDs by UUIDs via the registry
pub async fn batch_lookup_ids_by_uuids(
	table: &str,
	uuids: std::collections::HashSet<uuid::Uuid>,
	db: Arc<DatabaseConnection>,
) -> Result<std::collections::HashMap<uuid::Uuid, i32>, ApplyError> {
	let lookup_fn = {
		let registry = SYNCABLE_REGISTRY.read().await;
		let reg = registry
			.get(table)
			.ok_or_else(|| ApplyError::UnknownModel(table.to_string()))?;
		reg.fk_batch_lookup_ids_fn
			.ok_or_else(|| ApplyError::MissingFkLookup(table.to_string()))?
	};

	lookup_fn(uuids, db)
		.await
		.map_err(|e| ApplyError::DatabaseError(e.to_string()))
}

/// Batch lookup UUIDs by local IDs via the registry
pub async fn batch_lookup_uuids_by_ids(
	table: &str,
	ids: std::collections::HashSet<i32>,
	db: Arc<DatabaseConnection>,
) -> Result<std::collections::HashMap<i32, uuid::Uuid>, ApplyError> {
	let lookup_fn = {
		let registry = SYNCABLE_REGISTRY.read().await;
		let reg = registry
			.get(table)
			.ok_or_else(|| ApplyError::UnknownModel(table.to_string()))?;
		reg.fk_batch_lookup_uuids_fn
			.ok_or_else(|| ApplyError::MissingFkLookup(table.to_string()))?
	};

	lookup_fn(ids, db)
		.await
		.map_err(|e| ApplyError::DatabaseError(e.to_string()))
}

/// Check if a model has FK lookups registered
pub async fn has_fk_lookups(table: &str) -> bool {
	let registry = SYNCABLE_REGISTRY.read().await;
	registry
		.get(table)
		.map(|reg| reg.fk_lookup_id_fn.is_some())
		.unwrap_or(false)
}

/// Execute post-backfill rebuild for all models that have it registered
pub async fn run_post_backfill_rebuilds(db: Arc<DatabaseConnection>) -> Result<(), ApplyError> {
	let rebuild_fns: Vec<(String, PostBackfillRebuildFn)> = {
		let registry = SYNCABLE_REGISTRY.read().await;
		registry
			.iter()
			.filter_map(|(model, reg)| reg.post_backfill_rebuild_fn.map(|f| (model.clone(), f)))
			.collect()
	};

	for (model_type, rebuild_fn) in rebuild_fns {
		tracing::debug!(model = %model_type, "Running post-backfill rebuild");
		rebuild_fn(db.clone()).await.map_err(|e| {
			ApplyError::DatabaseError(format!("{} rebuild failed: {}", model_type, e))
		})?;
	}

	Ok(())
}

/// Errors that can occur when applying sync entries
#[derive(Debug, thiserror::Error)]
pub enum ApplyError {
	#[error("Unknown model type: {0}")]
	UnknownModel(String),

	#[error("Missing FK lookup function for table: {0}")]
	MissingFkLookup(String),

	#[error("Wrong sync type for model {model}: expected {expected}, got {got}")]
	WrongSyncType {
		model: String,
		expected: String,
		got: String,
	},

	#[error("Missing apply function for model: {0}")]
	MissingApplyFunction(String),

	#[error("Missing query function for model: {0}")]
	MissingQueryFunction(String),

	#[error("Missing deletion handler for model: {0}")]
	MissingDeletionHandler(String),

	#[error("Database error: {0}")]
	DatabaseError(String),
}

/// Compute sync order based on model dependencies
///
/// Performs topological sort on all registered models using their declared dependencies
/// to ensure foreign key constraints are never violated during sync.
///
/// # Returns
/// Ordered list of model names where dependencies always come before dependents
///
/// # Errors
/// Returns error if circular dependencies are detected or if a model declares
/// a dependency on a non-existent model.
pub async fn compute_registry_sync_order() -> Result<Vec<String>, super::DependencyError> {
	let registry_deps: Vec<(&str, &'static [&'static str])> = {
		let registry = SYNCABLE_REGISTRY.read().await;
		registry
			.iter()
			.filter_map(|(_, reg)| reg.sync_depends_on_fn.map(|f| (reg.model_type, f())))
			.collect()
	};

	super::dependency_graph::compute_sync_order(registry_deps.into_iter())
}

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn test_registry_initialization() {
		// Access registry to trigger initialization
		let registry = SYNCABLE_REGISTRY.read().await;

		// Print all registered models for debugging
		let mut models: Vec<_> = registry.keys().cloned().collect();
		models.sort();
		println!("Registered syncable models ({}):", models.len());
		for model in &models {
			let reg = registry.get(model).unwrap();
			let sync_type = if reg.is_device_owned { "device-owned" } else { "shared" };
			println!("  - {} ({})", model, sync_type);
		}

		// Verify location is registered as device-owned
		assert!(registry.contains_key("location"));
		let location_reg = registry.get("location").unwrap();
		assert_eq!(location_reg.model_type, "location");
		assert_eq!(location_reg.table_name, "locations");
		assert!(location_reg.is_device_owned);
		assert!(location_reg.state_apply_fn.is_some());
		assert!(location_reg.shared_apply_fn.is_none());

		// Verify tag is registered as shared
		assert!(registry.contains_key("tag"));
		let tag_reg = registry.get("tag").unwrap();
		assert_eq!(tag_reg.model_type, "tag");
		assert_eq!(tag_reg.table_name, "tag");
		assert!(!tag_reg.is_device_owned);
		assert!(tag_reg.state_apply_fn.is_none());
		assert!(tag_reg.shared_apply_fn.is_some());

		// Verify mime_type is registered as shared
		assert!(
			registry.contains_key("mime_type"),
			"mime_type should be registered but was not found. Registered models: {:?}",
			models
		);
		let mime_type_reg = registry.get("mime_type").unwrap();
		assert_eq!(mime_type_reg.model_type, "mime_type");
		assert_eq!(mime_type_reg.table_name, "mime_types");
		assert!(!mime_type_reg.is_device_owned);
		assert!(mime_type_reg.shared_apply_fn.is_some());
	}

	#[tokio::test]
	async fn test_registry_helpers() {
		// Trigger initialization
		let _ = SYNCABLE_REGISTRY.read().await;

		assert_eq!(get_table_name("location").await, Some("locations"));
		assert_eq!(get_table_name("tag").await, Some("tag"));
		assert_eq!(get_table_name("nonexistent").await, None);

		assert!(is_device_owned("location").await);
		assert!(!is_device_owned("tag").await);
		assert!(!is_device_owned("nonexistent").await); // Returns false for unknown
	}

	#[tokio::test]
	async fn test_sync_order_computation() {
		let order = compute_registry_sync_order().await.unwrap();

		// Verify key models are present
		assert!(order.contains(&"device".to_string()));
		assert!(order.contains(&"location".to_string()));
		assert!(order.contains(&"entry".to_string()));
		assert!(order.contains(&"tag".to_string()));
		assert!(order.contains(&"collection".to_string()));
		assert!(order.contains(&"space".to_string()));

		// Verify dependency ordering
		let device_idx = order.iter().position(|m| m == "device").unwrap();
		let location_idx = order.iter().position(|m| m == "location").unwrap();
		let entry_idx = order.iter().position(|m| m == "entry").unwrap();
		let collection_idx = order.iter().position(|m| m == "collection").unwrap();
		let collection_entry_idx = order.iter().position(|m| m == "collection_entry").unwrap();
		let space_idx = order.iter().position(|m| m == "space").unwrap();
		let space_group_idx = order.iter().position(|m| m == "space_group").unwrap();
		let space_item_idx = order.iter().position(|m| m == "space_item").unwrap();

		// Device must come before location
		assert!(
			device_idx < location_idx,
			"device must sync before location"
		);

		// Note: location and entry have a circular relationship (location.entry_id → entry, entries belong to locations)
		// This is handled by making location.entry_id nullable during sync, so no ordering constraint is enforced

		// M2M dependencies
		assert!(
			collection_idx < collection_entry_idx,
			"collection must sync before collection_entry"
		);
		assert!(
			entry_idx < collection_entry_idx,
			"entry must sync before collection_entry"
		);

		// Space hierarchy
		assert!(
			space_idx < space_group_idx,
			"space must sync before space_group"
		);
		assert!(
			space_group_idx < space_item_idx,
			"space_group must sync before space_item"
		);
	}
}
