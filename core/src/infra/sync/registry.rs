//! Syncable model registry
//!
//! Provides a runtime registry of all syncable models for dynamic dispatch.
//! This enables the sync applier to deserialize and apply changes without
//! knowing the concrete model type at compile time.

use super::{SharedChangeEntry, Syncable};
use sea_orm::DatabaseConnection;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::RwLock;

use once_cell::sync::Lazy;

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
	Option<uuid::Uuid>,                                      // device_id filter
	Option<chrono::DateTime<chrono::Utc>>,                   // since watermark
	Option<(chrono::DateTime<chrono::Utc>, uuid::Uuid)>,     // cursor for pagination
	usize,                                                    // batch_size
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
		}
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
		SyncableModelRegistration::device_owned(model_type, table_name, apply_fn, query_fn, delete_fn),
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

/// Initialize registry with all syncable models
///
/// This is completely domain-agnostic - it just routes to the Syncable trait implementations.
/// All domain-specific logic lives in the entity implementations, not here.
fn initialize_registry() -> HashMap<String, SyncableModelRegistration> {
	use crate::infra::db::entities::{
		audit_log, collection, collection_entry, content_identity, device, entry, location, sidecar,
		tag, tag_relationship, user_metadata, user_metadata_tag, volume,
	};

	let mut registry = HashMap::new();

	// Device-owned models (state-based sync)
	// Just function pointers - no domain logic here
	registry.insert(
		"location".to_string(),
		SyncableModelRegistration::device_owned(
			"location",
			"locations",
			|data, db| {
				Box::pin(
					async move { location::Model::apply_state_change(data, db.as_ref()).await },
				)
			},
			|device_id, since, cursor, batch_size, db| {
				Box::pin(async move {
					location::Model::query_for_sync(device_id, since, cursor, batch_size, db.as_ref()).await
				})
			},
			Some(|uuid, db| {
				Box::pin(async move { location::Model::apply_deletion(uuid, db.as_ref()).await })
			}),
		),
	);

	registry.insert(
		"volume".to_string(),
		SyncableModelRegistration::device_owned(
			"volume",
			"volumes",
			|data, db| {
				Box::pin(async move { volume::Model::apply_state_change(data, db.as_ref()).await })
			},
			|device_id, since, cursor, batch_size, db| {
				Box::pin(async move {
					volume::Model::query_for_sync(device_id, since, cursor, batch_size, db.as_ref()).await
				})
			},
			Some(|uuid, db| {
				Box::pin(async move { volume::Model::apply_deletion(uuid, db.as_ref()).await })
			}),
		),
	);

	registry.insert(
		"entry".to_string(),
		SyncableModelRegistration::device_owned(
			"entry",
			"entries",
			|data, db| {
				Box::pin(async move { entry::Model::apply_state_change(data, db.as_ref()).await })
			},
			|device_id, since, cursor, batch_size, db| {
				Box::pin(async move {
					entry::Model::query_for_sync(device_id, since, cursor, batch_size, db.as_ref()).await
				})
			},
			Some(|uuid, db| {
				Box::pin(async move { entry::Model::apply_deletion(uuid, db.as_ref()).await })
			}),
		),
	);

	registry.insert(
		"device".to_string(),
		SyncableModelRegistration::device_owned(
			"device",
			"devices",
			|data, db| {
				Box::pin(async move { device::Model::apply_state_change(data, db.as_ref()).await })
			},
			|device_id, since, cursor, batch_size, db| {
				Box::pin(async move {
					device::Model::query_for_sync(device_id, since, cursor, batch_size, db.as_ref()).await
				})
			},
			None, // Devices don't support deletion sync
		),
	);

	// Shared models (log-based sync with backfill support)
	registry.insert(
		"tag".to_string(),
		SyncableModelRegistration::shared_with_query(
			"tag",
			"tag",
			|entry, db| {
				Box::pin(async move { tag::Model::apply_shared_change(entry, db.as_ref()).await })
			},
			|device_id, since, cursor, batch_size, db| {
				Box::pin(async move {
					tag::Model::query_for_sync(device_id, since, cursor, batch_size, db.as_ref()).await
				})
			},
		),
	);

	registry.insert(
		"collection".to_string(),
		SyncableModelRegistration::shared_with_query(
			"collection",
			"collection",
			|entry, db| {
				Box::pin(async move {
					collection::Model::apply_shared_change(entry, db.as_ref()).await
				})
			},
			|device_id, since, cursor, batch_size, db| {
				Box::pin(async move {
					collection::Model::query_for_sync(device_id, since, cursor, batch_size, db.as_ref())
						.await
				})
			},
		),
	);

	registry.insert(
		"content_identity".to_string(),
		SyncableModelRegistration::shared_with_query(
			"content_identity",
			"content_identities",
			|entry, db| {
				Box::pin(async move {
					content_identity::Model::apply_shared_change(entry, db.as_ref()).await
				})
			},
			|device_id, since, cursor, batch_size, db| {
				Box::pin(async move {
					content_identity::Model::query_for_sync(
						device_id,
						since,
						cursor,
						batch_size,
						db.as_ref(),
					)
					.await
				})
			},
		),
	);

	registry.insert(
		"user_metadata".to_string(),
		SyncableModelRegistration::shared_with_query(
			"user_metadata",
			"user_metadata",
			|entry, db| {
				Box::pin(async move {
					user_metadata::Model::apply_shared_change(entry, db.as_ref()).await
				})
			},
			|device_id, since, cursor, batch_size, db| {
				Box::pin(async move {
					user_metadata::Model::query_for_sync(
						device_id,
						since,
						cursor,
						batch_size,
						db.as_ref(),
					)
					.await
				})
			},
		),
	);

	// Many-to-many junction tables (shared models)
	registry.insert(
		"collection_entry".to_string(),
		SyncableModelRegistration::shared_with_query(
			"collection_entry",
			"collection_entry",
			|entry, db| {
				Box::pin(async move {
					collection_entry::Model::apply_shared_change(entry, db.as_ref()).await
				})
			},
			|device_id, since, cursor, batch_size, db| {
				Box::pin(async move {
					collection_entry::Model::query_for_sync(
						device_id,
						since,
						cursor,
						batch_size,
						db.as_ref(),
					)
					.await
				})
			},
		),
	);

	registry.insert(
		"user_metadata_tag".to_string(),
		SyncableModelRegistration::shared_with_query(
			"user_metadata_tag",
			"user_metadata_tag",
			|entry, db| {
				Box::pin(async move {
					user_metadata_tag::Model::apply_shared_change(entry, db.as_ref()).await
				})
			},
			|device_id, since, cursor, batch_size, db| {
				Box::pin(async move {
					user_metadata_tag::Model::query_for_sync(
						device_id,
						since,
						cursor,
						batch_size,
						db.as_ref(),
					)
					.await
				})
			},
		),
	);

	registry.insert(
		"tag_relationship".to_string(),
		SyncableModelRegistration::shared_with_query(
			"tag_relationship",
			"tag_relationship",
			|entry, db| {
				Box::pin(async move {
					tag_relationship::Model::apply_shared_change(entry, db.as_ref()).await
				})
			},
			|device_id, since, cursor, batch_size, db| {
				Box::pin(async move {
					tag_relationship::Model::query_for_sync(
						device_id,
						since,
						cursor,
						batch_size,
						db.as_ref(),
					)
					.await
				})
			},
		),
	);

	registry.insert(
		"audit_log".to_string(),
		SyncableModelRegistration::shared_with_query(
			"audit_log",
			"audit_log",
			|entry, db| {
				Box::pin(async move {
					audit_log::Model::apply_shared_change(entry, db.as_ref()).await
				})
			},
			|device_id, since, cursor, batch_size, db| {
				Box::pin(async move {
					audit_log::Model::query_for_sync(
						device_id,
						since,
						cursor,
						batch_size,
						db.as_ref(),
					)
					.await
				})
			},
		),
	);

	registry.insert(
		"sidecar".to_string(),
		SyncableModelRegistration::shared_with_query(
			"sidecar",
			"sidecars",
			|entry, db| {
				Box::pin(async move {
					sidecar::Model::apply_shared_change(entry, db.as_ref()).await
				})
			},
			|device_id, since, cursor, batch_size, db| {
				Box::pin(async move {
					sidecar::Model::query_for_sync(
						device_id,
						since,
						cursor,
						batch_size,
						db.as_ref(),
					)
					.await
				})
			},
		),
	);

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

/// Check if model is device-owned
pub async fn is_device_owned(model_type: &str) -> bool {
	SYNCABLE_REGISTRY
		.read()
		.await
		.get(model_type)
		.map(|reg| reg.is_device_owned)
		.unwrap_or(false)
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
) -> Result<HashMap<String, Vec<(uuid::Uuid, serde_json::Value, chrono::DateTime<chrono::Utc>)>>, ApplyError> {
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

/// Errors that can occur when applying sync entries
#[derive(Debug, thiserror::Error)]
pub enum ApplyError {
	#[error("Unknown model type: {0}")]
	UnknownModel(String),

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
///
/// # Example
/// ```ignore
/// let order = compute_registry_sync_order().await?;
/// // order = ["device", "location", "entry", "tag"]
/// // Devices sync first, then locations, then entries, etc.
/// ```
pub async fn compute_registry_sync_order() -> Result<Vec<String>, super::DependencyError> {
	use crate::infra::db::entities::{
		audit_log, collection, collection_entry, content_identity, device, entry, location, tag,
		tag_relationship, user_metadata, user_metadata_tag, volume,
	};

	// Build iterator of (model_name, dependencies)
	let models = vec![
		(device::Model::SYNC_MODEL, device::Model::sync_depends_on()),
		(
			location::Model::SYNC_MODEL,
			location::Model::sync_depends_on(),
		),
		(volume::Model::SYNC_MODEL, volume::Model::sync_depends_on()),
		(entry::Model::SYNC_MODEL, entry::Model::sync_depends_on()),
		(tag::Model::SYNC_MODEL, tag::Model::sync_depends_on()),
		(
			collection::Model::SYNC_MODEL,
			collection::Model::sync_depends_on(),
		),
		(
			content_identity::Model::SYNC_MODEL,
			content_identity::Model::sync_depends_on(),
		),
		(
			user_metadata::Model::SYNC_MODEL,
			user_metadata::Model::sync_depends_on(),
		),
		(
			collection_entry::Model::SYNC_MODEL,
			collection_entry::Model::sync_depends_on(),
		),
		(
			user_metadata_tag::Model::SYNC_MODEL,
			user_metadata_tag::Model::sync_depends_on(),
		),
		(
			tag_relationship::Model::SYNC_MODEL,
			tag_relationship::Model::sync_depends_on(),
		),
		(
			audit_log::Model::SYNC_MODEL,
			audit_log::Model::sync_depends_on(),
		),
	];

	super::dependency_graph::compute_sync_order(models.into_iter())
}

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn test_registry_initialization() {
		// Access registry to trigger initialization
		let registry = SYNCABLE_REGISTRY.read().await;

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

		// Verify all models are present (8 base + 3 M2M + audit_log)
		assert_eq!(order.len(), 12);
		assert!(order.contains(&"device".to_string()));
		assert!(order.contains(&"location".to_string()));
		assert!(order.contains(&"volume".to_string()));
		assert!(order.contains(&"entry".to_string()));
		assert!(order.contains(&"tag".to_string()));
		assert!(order.contains(&"collection".to_string()));
		assert!(order.contains(&"content_identity".to_string()));
		assert!(order.contains(&"user_metadata".to_string()));
		assert!(order.contains(&"collection_entry".to_string()));
		assert!(order.contains(&"user_metadata_tag".to_string()));
		assert!(order.contains(&"tag_relationship".to_string()));
		assert!(order.contains(&"audit_log".to_string()));

		// Verify dependency ordering
		let device_idx = order.iter().position(|m| m == "device").unwrap();
		let location_idx = order.iter().position(|m| m == "location").unwrap();
		let volume_idx = order.iter().position(|m| m == "volume").unwrap();
		let entry_idx = order.iter().position(|m| m == "entry").unwrap();
		let collection_idx = order.iter().position(|m| m == "collection").unwrap();
		let collection_entry_idx = order.iter().position(|m| m == "collection_entry").unwrap();
		let tag_idx = order.iter().position(|m| m == "tag").unwrap();
		let user_metadata_idx = order.iter().position(|m| m == "user_metadata").unwrap();
		let user_metadata_tag_idx = order.iter().position(|m| m == "user_metadata_tag").unwrap();
		let tag_relationship_idx = order.iter().position(|m| m == "tag_relationship").unwrap();

		// Device must come before location
		assert!(
			device_idx < location_idx,
			"device must sync before location"
		);

		// Device must come before volume
		assert!(device_idx < volume_idx, "device must sync before volume");

		// Location must come before entry
		assert!(location_idx < entry_idx, "location must sync before entry");

		// M2M dependencies
		assert!(
			collection_idx < collection_entry_idx,
			"collection must sync before collection_entry"
		);
		assert!(
			entry_idx < collection_entry_idx,
			"entry must sync before collection_entry"
		);
		assert!(
			user_metadata_idx < user_metadata_tag_idx,
			"user_metadata must sync before user_metadata_tag"
		);
		assert!(
			tag_idx < user_metadata_tag_idx,
			"tag must sync before user_metadata_tag"
		);
		assert!(
			tag_idx < tag_relationship_idx,
			"tag must sync before tag_relationship"
		);
	}
}
