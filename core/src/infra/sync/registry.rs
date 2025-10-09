//! Syncable model registry
//!
//! Provides a runtime registry of all syncable models for dynamic dispatch.
//! This enables the sync applier to deserialize and apply changes without
//! knowing the concrete model type at compile time.

use super::Syncable;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::RwLock;

/// Registry of syncable models
///
/// Maps model_type strings (e.g., "album", "tag") to their registration info.
pub static SYNCABLE_REGISTRY: Lazy<RwLock<HashMap<String, SyncableModelRegistration>>> =
	Lazy::new(|| RwLock::new(HashMap::new()));

/// Registration information for a syncable model
pub struct SyncableModelRegistration {
	/// Model type identifier
	pub model_type: &'static str,
	// TODO: Function pointer to deserialize and apply sync entry
	// Will be implemented when we add the apply logic
}

impl SyncableModelRegistration {
	/// Create a new registration
	pub fn new(model_type: &'static str) -> Self {
		Self { model_type }
	}
}

/// Register a syncable model type
pub fn register_model(model_type: &'static str) {
	let mut registry = SYNCABLE_REGISTRY.write().unwrap();
	registry.insert(
		model_type.to_string(),
		SyncableModelRegistration::new(model_type),
	);
}

/// Get the registry (for inspection)
pub fn get_registry() -> HashMap<String, SyncableModelRegistration> {
	SYNCABLE_REGISTRY
		.read()
		.unwrap()
		.iter()
		.map(|(k, v)| (k.clone(), SyncableModelRegistration::new(v.model_type)))
		.collect()
}

/// Apply a sync entry (STUB - will be implemented)
///
/// In the new architecture, this will:
/// 1. Check if model is device-owned (state-based) or shared (log-based)
/// 2. Apply appropriate merge strategy
/// 3. Update database
pub async fn apply_sync_entry(_model_type: &str, _data: serde_json::Value) -> Result<(), String> {
	// TODO: Implement when we add sync applier logic
	warn!("apply_sync_entry not yet implemented in leaderless architecture");
	Ok(())
}

use tracing::warn;

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_registry() {
		register_model("test_model");

		let registry = get_registry();
		assert!(registry.contains_key("test_model"));
	}
}
