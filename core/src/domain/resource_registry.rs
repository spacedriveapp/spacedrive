//! Resource Registry - Static registry of all resources
//!
//! This module provides a registry for all resources (both simple and virtual),
//! allowing generic routing and construction without hardcoded match statements.
//!
//! ## Simple Resources
//! Backed by a single database table with no dependencies.
//! - Space, SpaceGroup, SpaceItem, LocationInfo
//!
//! ## Virtual Resources
//! Computed from multiple database tables with dependencies.
//! - File (from Entry, ContentIdentity, Sidecar, etc.)
//! - SpaceLayout (from Space, SpaceGroup, SpaceItem)

use crate::common::errors::Result;
use crate::domain::resource::Identifiable;
use crate::domain::{File, Space, SpaceGroup, SpaceItem, SpaceLayout};
use crate::ops::locations::list::output::LocationInfo;
use once_cell::sync::Lazy;
use sea_orm::DatabaseConnection;
use std::future::Future;
use std::pin::Pin;
use uuid::Uuid;

/// Information about a registered virtual resource
///
/// This struct holds the metadata and function pointers needed to
/// route dependency changes and construct virtual resources generically.
pub struct VirtualResourceInfo {
	/// Resource type identifier (e.g., "file", "space_layout")
	pub resource_type: &'static str,

	/// List of dependency resource types
	pub dependencies: &'static [&'static str],

	/// Function to route a dependency change to affected virtual resource IDs
	pub router: for<'a> fn(
		&'a DatabaseConnection,
		&'a str,
		Uuid,
	) -> Pin<Box<dyn Future<Output = Result<Vec<Uuid>>> + Send + 'a>>,

	/// Function to construct virtual resources from IDs
	pub constructor:
		for<'a> fn(
			&'a DatabaseConnection,
			&'a [Uuid],
		) -> Pin<Box<dyn Future<Output = Result<Vec<serde_json::Value>>> + Send + 'a>>,

	/// Static list of fields that should not be merged (for metadata)
	pub no_merge_fields: &'static [&'static str],
}

/// Static registry of all resources (simple and virtual)
static VIRTUAL_RESOURCES: Lazy<Vec<VirtualResourceInfo>> = Lazy::new(|| {
	vec![
		// === Virtual Resources (multi-table) ===
		VirtualResourceInfo {
			resource_type: File::resource_type(),
			dependencies: File::sync_dependencies(),
			router: |db, dep_type, dep_id| {
				Box::pin(async move { File::route_from_dependency(db, dep_type, dep_id).await })
			},
			constructor: |db, ids| {
				Box::pin(async move {
					let resources = File::from_ids(db, ids).await?;
					resources
						.into_iter()
						.map(|r| {
							serde_json::to_value(&r).map_err(|e| {
								crate::common::errors::CoreError::Other(anyhow::anyhow!(
									"Failed to serialize File: {}",
									e
								))
							})
						})
						.collect::<Result<Vec<_>>>()
				})
			},
			no_merge_fields: File::no_merge_fields(),
		},
		VirtualResourceInfo {
			resource_type: SpaceLayout::resource_type(),
			dependencies: SpaceLayout::sync_dependencies(),
			router: |db, dep_type, dep_id| {
				Box::pin(
					async move { SpaceLayout::route_from_dependency(db, dep_type, dep_id).await },
				)
			},
			constructor: |db, ids| {
				Box::pin(async move {
					let resources = SpaceLayout::from_ids(db, ids).await?;
					resources
						.into_iter()
						.map(|r| {
							serde_json::to_value(&r).map_err(|e| {
								crate::common::errors::CoreError::Other(anyhow::anyhow!(
									"Failed to serialize SpaceLayout: {}",
									e
								))
							})
						})
						.collect::<Result<Vec<_>>>()
				})
			},
			no_merge_fields: SpaceLayout::no_merge_fields(),
		},
		// === Simple Resources (single table) ===
		VirtualResourceInfo {
			resource_type: Space::resource_type(),
			dependencies: &[], // Simple resources have no dependencies
			router: |_db, _dep_type, _dep_id| {
				Box::pin(async move { Ok(vec![]) }) // Simple resources don't route
			},
			constructor: |db, ids| {
				Box::pin(async move {
					let resources = Space::from_ids(db, ids).await?;
					resources
						.into_iter()
						.map(|r| {
							serde_json::to_value(&r).map_err(|e| {
								crate::common::errors::CoreError::Other(anyhow::anyhow!(
									"Failed to serialize Space: {}",
									e
								))
							})
						})
						.collect::<Result<Vec<_>>>()
				})
			},
			no_merge_fields: Space::no_merge_fields(),
		},
		VirtualResourceInfo {
			resource_type: SpaceGroup::resource_type(),
			dependencies: &[],
			router: |_db, _dep_type, _dep_id| Box::pin(async move { Ok(vec![]) }),
			constructor: |db, ids| {
				Box::pin(async move {
					let resources = SpaceGroup::from_ids(db, ids).await?;
					resources
						.into_iter()
						.map(|r| {
							serde_json::to_value(&r).map_err(|e| {
								crate::common::errors::CoreError::Other(anyhow::anyhow!(
									"Failed to serialize SpaceGroup: {}",
									e
								))
							})
						})
						.collect::<Result<Vec<_>>>()
				})
			},
			no_merge_fields: SpaceGroup::no_merge_fields(),
		},
		VirtualResourceInfo {
			resource_type: SpaceItem::resource_type(),
			dependencies: &[],
			router: |_db, _dep_type, _dep_id| Box::pin(async move { Ok(vec![]) }),
			constructor: |db, ids| {
				Box::pin(async move {
					let resources = SpaceItem::from_ids(db, ids).await?;
					resources
						.into_iter()
						.map(|r| {
							serde_json::to_value(&r).map_err(|e| {
								crate::common::errors::CoreError::Other(anyhow::anyhow!(
									"Failed to serialize SpaceItem: {}",
									e
								))
							})
						})
						.collect::<Result<Vec<_>>>()
				})
			},
			no_merge_fields: SpaceItem::no_merge_fields(),
		},
		VirtualResourceInfo {
			resource_type: LocationInfo::resource_type(),
			dependencies: &[],
			router: |_db, _dep_type, _dep_id| Box::pin(async move { Ok(vec![]) }),
			constructor: |db, ids| {
				Box::pin(async move {
					let resources = LocationInfo::from_ids(db, ids).await?;
					resources
						.into_iter()
						.map(|r| {
							serde_json::to_value(&r).map_err(|e| {
								crate::common::errors::CoreError::Other(anyhow::anyhow!(
									"Failed to serialize LocationInfo: {}",
									e
								))
							})
						})
						.collect::<Result<Vec<_>>>()
				})
			},
			no_merge_fields: LocationInfo::no_merge_fields(),
		},
	]
});

/// Get all registered virtual resources
pub fn all_virtual_resources() -> &'static [VirtualResourceInfo] {
	&VIRTUAL_RESOURCES
}

/// Find a virtual resource by type
pub fn find_by_type(resource_type: &str) -> Option<&'static VirtualResourceInfo> {
	VIRTUAL_RESOURCES
		.iter()
		.find(|r| r.resource_type == resource_type)
}

/// Find all virtual resources that depend on a given resource type
pub fn find_dependents(dependency_type: &str) -> Vec<&'static VirtualResourceInfo> {
	VIRTUAL_RESOURCES
		.iter()
		.filter(|r| r.dependencies.contains(&dependency_type))
		.collect()
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_registry_has_resources() {
		let resources = all_virtual_resources();
		assert_eq!(
			resources.len(),
			6,
			"Expected 6 registered resources (File, SpaceLayout, Space, SpaceGroup, SpaceItem, LocationInfo), got {}",
			resources.len()
		);
	}

	#[test]
	fn test_find_simple_resources() {
		// Test that simple resources are registered
		assert!(
			find_by_type("space").is_some(),
			"Space should be registered"
		);
		assert!(
			find_by_type("space_group").is_some(),
			"SpaceGroup should be registered"
		);
		assert!(
			find_by_type("space_item").is_some(),
			"SpaceItem should be registered"
		);
		assert!(
			find_by_type("location").is_some(),
			"Location should be registered"
		);
	}

	#[test]
	fn test_simple_resources_have_no_dependencies() {
		// Simple resources should have empty dependencies
		let simple_types = ["space", "space_group", "space_item", "location"];

		for resource_type in simple_types {
			if let Some(info) = find_by_type(resource_type) {
				assert!(
					info.dependencies.is_empty(),
					"{} should have no dependencies (it's a simple resource)",
					resource_type
				);
			}
		}
	}

	#[test]
	fn test_find_file_resource() {
		let file_info = find_by_type("file");
		assert!(file_info.is_some(), "File resource should be registered");

		if let Some(info) = file_info {
			assert_eq!(info.resource_type, "file");
			assert!(info.dependencies.contains(&"entry"));
			assert!(info.dependencies.contains(&"content_identity"));
		}
	}

	#[test]
	fn test_find_space_layout_resource() {
		let layout_info = find_by_type("space_layout");
		assert!(
			layout_info.is_some(),
			"SpaceLayout resource should be registered"
		);

		if let Some(info) = layout_info {
			assert_eq!(info.resource_type, "space_layout");
			assert!(info.dependencies.contains(&"space"));
			assert!(info.dependencies.contains(&"space_group"));
			assert!(info.dependencies.contains(&"space_item"));
		}
	}

	#[test]
	fn test_find_dependents_of_entry() {
		let dependents = find_dependents("entry");
		assert!(
			!dependents.is_empty(),
			"Entry should have at least one dependent (File)"
		);

		let has_file = dependents.iter().any(|r| r.resource_type == "file");
		assert!(has_file, "File should depend on entry");
	}
}
