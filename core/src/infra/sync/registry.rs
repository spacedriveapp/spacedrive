//! Syncable model registry
//!
//! Automatically registers models that implement Syncable at compile-time
//! using the `inventory` crate (same pattern as action/query registry).

use super::SyncLogEntry;
use sea_orm::DatabaseConnection;
use std::collections::HashMap;
use std::sync::OnceLock;

/// Function signature for applying a sync entry
pub type ApplyFn = fn(
	&SyncLogEntry,
	&DatabaseConnection,
) -> std::pin::Pin<
	Box<
		dyn std::future::Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>>
			+ Send,
	>,
>;

/// Syncable model registration
pub struct SyncableModelRegistration {
	/// Model type identifier (e.g., "location", "tag")
	pub model_type: &'static str,

	/// Function to apply sync entries for this model
	pub apply_fn: ApplyFn,
}

inventory::collect!(SyncableModelRegistration);

/// Global registry of syncable models
static SYNCABLE_REGISTRY: OnceLock<HashMap<&'static str, ApplyFn>> = OnceLock::new();

/// Get the syncable model registry
pub fn get_registry() -> &'static HashMap<&'static str, ApplyFn> {
	SYNCABLE_REGISTRY.get_or_init(|| {
		let mut registry = HashMap::new();

		for registration in inventory::iter::<SyncableModelRegistration> {
			registry.insert(registration.model_type, registration.apply_fn);
		}

		tracing::info!(
			model_count = registry.len(),
			"Syncable model registry initialized"
		);

		registry
	})
}

/// Apply a sync entry using the registry
///
/// Looks up the model type and calls its apply function.
pub async fn apply_sync_entry(
	entry: &SyncLogEntry,
	db: &DatabaseConnection,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
	let registry = get_registry();

	if let Some(apply_fn) = registry.get(entry.model_type.as_str()) {
		apply_fn(entry, db).await
	} else {
		Err(format!(
			"No sync handler registered for model type '{}'",
			entry.model_type
		)
		.into())
	}
}

/// Macro to register a syncable model
///
/// Usage:
/// ```rust,ignore
/// register_syncable_model!(location::Model);
/// ```
#[macro_export]
macro_rules! register_syncable_model {
	($model:ty) => {
		inventory::submit! {
			$crate::infra::sync::registry::SyncableModelRegistration {
				model_type: <$model as $crate::infra::sync::Syncable>::SYNC_MODEL,
				apply_fn: |entry, db| {
					Box::pin(async move {
						<$model as $crate::infra::sync::Syncable>::apply_sync_entry(entry, db).await
					})
				},
			}
		}
	};
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_registry_initialization() {
		let registry = get_registry();
		// Should have at least location registered
		assert!(registry.len() > 0);
	}
}
