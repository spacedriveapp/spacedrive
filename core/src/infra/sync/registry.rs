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
	Option<uuid::Uuid>,
	Option<chrono::DateTime<chrono::Utc>>,
	usize,
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

	/// Query function for state-based backfill (device-owned models)
	pub state_query_fn: Option<StateQueryFn>,
}

impl SyncableModelRegistration {
	/// Create a new registration for device-owned model
	pub fn device_owned(
		model_type: &'static str,
		table_name: &'static str,
		apply_fn: StateApplyFn,
		query_fn: StateQueryFn,
	) -> Self {
		Self {
			model_type,
			table_name,
			is_device_owned: true,
			state_apply_fn: Some(apply_fn),
			shared_apply_fn: None,
			state_query_fn: Some(query_fn),
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
		}
	}
}

/// Register a device-owned model with state-based apply and query functions
pub async fn register_device_owned(
	model_type: &'static str,
	table_name: &'static str,
	apply_fn: StateApplyFn,
	query_fn: StateQueryFn,
) {
	let mut registry = SYNCABLE_REGISTRY.write().await;
	registry.insert(
		model_type.to_string(),
		SyncableModelRegistration::device_owned(model_type, table_name, apply_fn, query_fn),
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
	use crate::infra::db::entities::{device, entry, location, tag};

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
			|device_id, since, batch_size, db| {
				Box::pin(async move {
					location::Model::query_for_sync(device_id, since, batch_size, db.as_ref()).await
				})
			},
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
			|device_id, since, batch_size, db| {
				Box::pin(async move {
					entry::Model::query_for_sync(device_id, since, batch_size, db.as_ref()).await
				})
			},
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
			|device_id, since, batch_size, db| {
				Box::pin(async move {
					device::Model::query_for_sync(device_id, since, batch_size, db.as_ref()).await
				})
			},
		),
	);

	// Shared models (log-based sync)
	registry.insert(
		"tag".to_string(),
		SyncableModelRegistration::shared("tag", "tag", |entry, db| {
			Box::pin(async move { tag::Model::apply_shared_change(entry, db.as_ref()).await })
		}),
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

	// Call the registered query function
	query_fn(device_id, since, batch_size, db)
		.await
		.map_err(|e| ApplyError::DatabaseError(e.to_string()))
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

	#[error("Database error: {0}")]
	DatabaseError(String),
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
}
