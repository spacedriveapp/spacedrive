//! Resource Registry - Dynamic registry of all resources
//!
//! This module provides a runtime registry of all resources (simple and virtual)
//! for dynamic dispatch. This enables the ResourceManager to emit events without
//! knowing the concrete resource type at compile time.
//!
//! Resources self-register using the `register_resource!` macro, which uses
//! the `inventory` crate to collect registrations at link time.
//!
//! ## Pattern
//!
//! This mirrors the Syncable registry pattern:
//! - Each resource implements `Identifiable`
//! - Each resource uses `register_resource!` macro to self-register
//! - ResourceManager dispatches via function pointers, not match statements

use crate::common::errors::Result;
use once_cell::sync::Lazy;
use sea_orm::DatabaseConnection;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use uuid::Uuid;

// =============================================================================
// Type Aliases for Function Pointers
// =============================================================================

/// Function to construct resource instances from IDs and serialize to JSON
pub type ConstructorFn =
	for<'a> fn(
		&'a DatabaseConnection,
		&'a [Uuid],
	) -> Pin<Box<dyn Future<Output = Result<Vec<serde_json::Value>>> + Send + 'a>>;

/// Function to route a dependency change to affected resource IDs
pub type RouterFn = for<'a> fn(
	&'a DatabaseConnection,
	&'a str,
	Uuid,
) -> Pin<Box<dyn Future<Output = Result<Vec<Uuid>>> + Send + 'a>>;

/// Function to get static resource metadata
pub type DependenciesFn = fn() -> &'static [&'static str];
pub type NoMergeFieldsFn = fn() -> &'static [&'static str];

// =============================================================================
// Inventory-based Registration
// =============================================================================

/// Entry submitted to inventory for resource registration.
/// Resources submit these via the `register_resource!` macro.
pub struct ResourceInventoryEntry {
	/// Function that builds the full registration
	pub build: fn() -> ResourceRegistration,
}

// Tell inventory about our entry type
inventory::collect!(ResourceInventoryEntry);

/// Registration information for a resource
pub struct ResourceRegistration {
	/// Resource type identifier (e.g., "file", "space", "location")
	pub resource_type: &'static str,

	/// List of dependency resource types (for virtual resources)
	/// Simple resources return empty slice.
	pub dependencies: &'static [&'static str],

	/// Function to route a dependency change to affected resource IDs
	/// Simple resources return empty vec.
	pub router: RouterFn,

	/// Function to construct resources from IDs and serialize to JSON
	pub constructor: ConstructorFn,

	/// Static list of fields that should not be merged (for metadata)
	pub no_merge_fields: &'static [&'static str],
}

impl ResourceRegistration {
	/// Create a new resource registration
	pub fn new(
		resource_type: &'static str,
		dependencies: &'static [&'static str],
		router: RouterFn,
		constructor: ConstructorFn,
		no_merge_fields: &'static [&'static str],
	) -> Self {
		Self {
			resource_type,
			dependencies,
			router,
			constructor,
			no_merge_fields,
		}
	}

	/// Create a registration for a simple resource (no dependencies)
	pub fn simple(
		resource_type: &'static str,
		constructor: ConstructorFn,
		no_merge_fields: &'static [&'static str],
	) -> Self {
		Self {
			resource_type,
			dependencies: &[],
			router: |_db, _dep_type, _dep_id| Box::pin(async move { Ok(vec![]) }),
			constructor,
			no_merge_fields,
		}
	}
}

// =============================================================================
// Registry Initialization
// =============================================================================

/// Static registry of all resources, initialized from inventory
static RESOURCE_REGISTRY: Lazy<HashMap<&'static str, ResourceRegistration>> = Lazy::new(|| {
	let mut registry = HashMap::new();

	for entry in inventory::iter::<ResourceInventoryEntry> {
		let registration = (entry.build)();
		registry.insert(registration.resource_type, registration);
	}

	tracing::debug!(
		"Resource registry initialized with {} resources: {:?}",
		registry.len(),
		registry.keys().collect::<Vec<_>>()
	);

	registry
});

// =============================================================================
// Public API
// =============================================================================

/// Get all registered resources
pub fn all_resources() -> impl Iterator<Item = &'static ResourceRegistration> {
	RESOURCE_REGISTRY.values()
}

/// Find a resource by type
pub fn find_by_type(resource_type: &str) -> Option<&'static ResourceRegistration> {
	RESOURCE_REGISTRY.get(resource_type)
}

/// Find all resources that depend on a given resource type
pub fn find_dependents(dependency_type: &str) -> Vec<&'static ResourceRegistration> {
	RESOURCE_REGISTRY
		.values()
		.filter(|r| r.dependencies.contains(&dependency_type))
		.collect()
}

/// Get all registered resource types
pub fn all_resource_types() -> Vec<&'static str> {
	RESOURCE_REGISTRY.keys().copied().collect()
}

// =============================================================================
// Registration Macro
// =============================================================================

/// Register a resource type with the resource registry.
///
/// This macro should be called in the module where the resource is defined.
/// It automatically implements the registry entry using the Identifiable trait.
///
/// # Usage
///
/// For simple resources (single table, no dependencies):
/// ```ignore
/// register_resource!(Space);
/// ```
///
/// For virtual resources (computed from multiple tables):
/// ```ignore
/// register_resource!(File, virtual);
/// ```
#[macro_export]
macro_rules! register_resource {
	// Simple resource (no dependencies, no routing)
	($resource:ty) => {
		inventory::submit! {
			$crate::domain::resource_registry::ResourceInventoryEntry {
				build: || {
					$crate::domain::resource_registry::ResourceRegistration::simple(
						<$resource as $crate::domain::resource::Identifiable>::resource_type(),
						|db, ids| {
							Box::pin(async move {
								let resources = <$resource as $crate::domain::resource::Identifiable>::from_ids(db, ids).await?;
								resources
									.into_iter()
									.map(|r| {
										serde_json::to_value(&r).map_err(|e| {
											$crate::common::errors::CoreError::Other(anyhow::anyhow!(
												"Failed to serialize {}: {}",
												<$resource as $crate::domain::resource::Identifiable>::resource_type(),
												e
											))
										})
									})
									.collect::<$crate::common::errors::Result<Vec<_>>>()
							})
						},
						<$resource as $crate::domain::resource::Identifiable>::no_merge_fields(),
					)
				}
			}
		}
	};

	// Virtual resource (with dependencies and routing)
	($resource:ty, virtual) => {
		inventory::submit! {
			$crate::domain::resource_registry::ResourceInventoryEntry {
				build: || {
					$crate::domain::resource_registry::ResourceRegistration::new(
						<$resource as $crate::domain::resource::Identifiable>::resource_type(),
						<$resource as $crate::domain::resource::Identifiable>::sync_dependencies(),
						|db, dep_type, dep_id| {
							Box::pin(async move {
								<$resource as $crate::domain::resource::Identifiable>::route_from_dependency(db, dep_type, dep_id).await
							})
						},
						|db, ids| {
							Box::pin(async move {
								let resources = <$resource as $crate::domain::resource::Identifiable>::from_ids(db, ids).await?;
								resources
									.into_iter()
									.map(|r| {
										serde_json::to_value(&r).map_err(|e| {
											$crate::common::errors::CoreError::Other(anyhow::anyhow!(
												"Failed to serialize {}: {}",
												<$resource as $crate::domain::resource::Identifiable>::resource_type(),
												e
											))
										})
									})
									.collect::<$crate::common::errors::Result<Vec<_>>>()
							})
						},
						<$resource as $crate::domain::resource::Identifiable>::no_merge_fields(),
					)
				}
			}
		}
	};
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_registry_initialization() {
		// Access registry to trigger initialization
		let count = RESOURCE_REGISTRY.len();
		// We should have at least some resources registered
		println!("Registered resources: {:?}", all_resource_types());
		// Note: actual count depends on which resources use register_resource!
	}

	#[test]
	fn test_find_by_type_unknown() {
		assert!(find_by_type("nonexistent_resource_type").is_none());
	}
}
