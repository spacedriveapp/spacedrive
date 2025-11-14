//! Resource Registry - Static registry of virtual resources
//!
//! This module provides a registry for virtual resources,
//! allowing generic routing and construction without hardcoded match statements.

use crate::common::errors::Result;
use crate::domain::resource::Identifiable;
use crate::domain::{File, SpaceLayout};
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
	pub constructor: for<'a> fn(
		&'a DatabaseConnection,
		&'a [Uuid],
	) -> Pin<Box<dyn Future<Output = Result<Vec<serde_json::Value>>> + Send + 'a>>,

	/// Static list of fields that should not be merged (for metadata)
	pub no_merge_fields: &'static [&'static str],
}

/// Static registry of all virtual resources
static VIRTUAL_RESOURCES: Lazy<Vec<VirtualResourceInfo>> = Lazy::new(|| {
	vec![
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
				Box::pin(async move {
					SpaceLayout::route_from_dependency(db, dep_type, dep_id).await
				})
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
			2,
			"Expected 2 registered virtual resources (File, SpaceLayout), got {}",
			resources.len()
		);
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
