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
use std::sync::{Arc, RwLock};

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
}

impl SyncableModelRegistration {
	/// Create a new registration for device-owned model
	pub fn device_owned(
		model_type: &'static str,
		table_name: &'static str,
		apply_fn: StateApplyFn,
	) -> Self {
		Self {
			model_type,
			table_name,
			is_device_owned: true,
			state_apply_fn: Some(apply_fn),
			shared_apply_fn: None,
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
		}
	}
}

/// Register a device-owned model with state-based apply function
pub fn register_device_owned(
	model_type: &'static str,
	table_name: &'static str,
	apply_fn: StateApplyFn,
) {
	let mut registry = SYNCABLE_REGISTRY.write().unwrap();
	registry.insert(
		model_type.to_string(),
		SyncableModelRegistration::device_owned(model_type, table_name, apply_fn),
	);
}

/// Register a shared model with log-based apply function
pub fn register_shared(
	model_type: &'static str,
	table_name: &'static str,
	apply_fn: SharedApplyFn,
) {
	let mut registry = SYNCABLE_REGISTRY.write().unwrap();
	registry.insert(
		model_type.to_string(),
		SyncableModelRegistration::shared(model_type, table_name, apply_fn),
	);
}

/// Initialize registry with all syncable models
fn initialize_registry() -> HashMap<String, SyncableModelRegistration> {
	use crate::infra::db::entities::{location, tag};

	let mut registry = HashMap::new();

	// Device-owned models (state-based sync)
	registry.insert(
		"location".to_string(),
		SyncableModelRegistration::device_owned("location", "locations", |data, db| {
			Box::pin(async move { location::Model::apply_state_change(data, &db).await })
		}),
	);

	// Shared models (log-based sync)
	registry.insert(
		"tag".to_string(),
		SyncableModelRegistration::shared("tag", "tag", |entry, db| {
			Box::pin(async move { tag::Model::apply_shared_change(entry, &db).await })
		}),
	);

	registry
}

/// Get table name for a model type
pub fn get_table_name(model_type: &str) -> Option<&'static str> {
	SYNCABLE_REGISTRY
		.read()
		.unwrap()
		.get(model_type)
		.map(|reg| reg.table_name)
}

/// Check if model is device-owned
pub fn is_device_owned(model_type: &str) -> bool {
	SYNCABLE_REGISTRY
		.read()
		.unwrap()
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
	let registry = SYNCABLE_REGISTRY.read().unwrap();
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

	let apply_fn = registration
		.state_apply_fn
		.ok_or_else(|| ApplyError::MissingApplyFunction(model_type.to_string()))?;

	// Drop the lock before calling async function
	drop(registry);

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
	let registry = SYNCABLE_REGISTRY.read().unwrap();
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

	let apply_fn = registration
		.shared_apply_fn
		.ok_or_else(|| ApplyError::MissingApplyFunction(entry.model_type.clone()))?;

	// Drop the lock before calling async function
	drop(registry);

	// Call the registered apply function
	apply_fn(entry, db)
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

	#[error("Database error: {0}")]
	DatabaseError(String),
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_registry_initialization() {
		// Access registry to trigger initialization
		let registry = SYNCABLE_REGISTRY.read().unwrap();

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

	#[test]
	fn test_registry_helpers() {
		// Trigger initialization
		let _ = SYNCABLE_REGISTRY.read().unwrap();

		assert_eq!(get_table_name("location"), Some("locations"));
		assert_eq!(get_table_name("tag"), Some("tag"));
		assert_eq!(get_table_name("nonexistent"), None);

		assert!(is_device_owned("location"));
		assert!(!is_device_owned("tag"));
		assert!(!is_device_owned("nonexistent")); // Returns false for unknown
	}
}
