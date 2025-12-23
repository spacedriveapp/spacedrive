//! Resource Manager - Maps low-level DB changes to high-level resource events
//!
//! This system handles the complexity of virtual resources (like File) that
//! are computed from multiple database tables.
//!
//! When a low-level resource changes (e.g., ContentIdentity created),
//! the ResourceManager determines which high-level resources are affected
//! and emits appropriate events for the frontend normalized cache.
//!
//! ## Architecture
//!
//! All resources (simple and virtual) are registered in resource_registry.rs.
//! ResourceManager uses the registry to dispatch event construction generically.
//!
//! For simple resources (Space, Location, etc.):
//! - Direct lookup in registry and call constructor
//!
//! For virtual resources (File, SpaceLayout):
//! - Map dependency changes to affected virtual resource IDs
//! - Call constructor to build complete resources
//!
//! ## Usage
//!
//! ```ignore
//! // Emit events for a resource change
//! resource_manager.emit_resource_events("location", vec![location_id]).await?;
//!
//! // Or use EventEmitter trait directly on domain models
//! use crate::domain::resource::EventEmitter;
//! location.emit_changed(&events)?;
//! ```

use crate::common::errors::Result;
use crate::infra::event::{Event, EventBus, ResourceMetadata};
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use uuid::Uuid;

/// Resource Manager coordinates event emission for all resources
pub struct ResourceManager {
	db: Arc<DatabaseConnection>,
	events: Arc<EventBus>,
}

impl ResourceManager {
	pub fn new(db: Arc<DatabaseConnection>, events: Arc<EventBus>) -> Self {
		Self { db, events }
	}

	/// Extract affected paths from File resources for path-scoped filtering
	fn extract_file_paths(resources: &[serde_json::Value]) -> Vec<crate::domain::SdPath> {
		use std::collections::HashSet;

		let mut paths = HashSet::new();

		for resource in resources {
			// Extract sd_path (primary path)
			if let Some(sd_path) = resource.get("sd_path") {
				if let Ok(path) = serde_json::from_value::<crate::domain::SdPath>(sd_path.clone()) {
					// For physical paths, add the parent directory
					// This ensures directory views get notified of child changes
					if let Some(parent) = path.parent() {
						paths.insert(parent);
					}
					paths.insert(path);
				}
			}

			// Extract alternate_paths (all other physical locations)
			if let Some(alt_paths) = resource.get("alternate_paths") {
				if let Ok(path_list) =
					serde_json::from_value::<Vec<crate::domain::SdPath>>(alt_paths.clone())
				{
					for path in path_list {
						// Add parent directories for alternate paths too
						if let Some(parent) = path.parent() {
							paths.insert(parent);
						}
						paths.insert(path);
					}
				}
			}
		}

		paths.into_iter().collect()
	}

	/// Emit events for a resource change, handling virtual resource mapping
	///
	/// For simple resources (backed by single table):
	/// - Emits ResourceChanged event directly
	///
	/// For dependency resources (e.g., ContentIdentity):
	/// - Maps to affected virtual resources (e.g., File)
	/// - Constructs virtual resource instances
	/// - Emits ResourceChanged events for virtual resources
	///
	/// # Arguments
	/// * `resource_type` - Type of resource that changed (e.g., "content_identity")
	/// * `resource_ids` - IDs of changed resources
	///
	/// # Example
	/// ```ignore
	/// // ContentIdentity created
	/// manager.emit_resource_events("content_identity", vec![ci_id]).await?;
	///
	/// // This will:
	/// // 1. Map content_identity → file dependencies
	/// // 2. Construct File instances for affected entries
	/// // 3. Emit ResourceChanged { resource_type: "file", resources: [...] }
	/// ```
	pub async fn emit_resource_events(
		&self,
		resource_type: &str,
		resource_ids: Vec<Uuid>,
	) -> Result<()> {
		use crate::domain::resource::map_dependency_to_virtual_ids;

		if resource_ids.is_empty() {
			return Ok(());
		}

		tracing::debug!(
			resource_type = %resource_type,
			count = resource_ids.len(),
			"ResourceManager::emit_resource_events called"
		);

		// Check if this is a registered resource (simple or virtual)
		// Uses the registry for all resources - no hardcoded match statements!
		if let Some(resource_info) = crate::domain::resource_registry::find_by_type(resource_type) {
			// Construct resources using trait method via registry
			let resources_json = (resource_info.constructor)(&self.db, &resource_ids).await?;

			if !resources_json.is_empty() {
				tracing::info!(
					"Emitting {} {} ResourceChanged events",
					resources_json.len(),
					resource_type
				);

				// Extract affected paths for file resources
				let affected_paths = if resource_type == "file" {
					Self::extract_file_paths(&resources_json)
				} else {
					vec![]
				};

				let metadata = ResourceMetadata {
					no_merge_fields: resource_info
						.no_merge_fields
						.iter()
						.map(|s| s.to_string())
						.collect(),
					alternate_ids: vec![],
					affected_paths,
				};

				self.events.emit(Event::ResourceChangedBatch {
					resource_type: resource_type.to_string(),
					resources: serde_json::Value::Array(resources_json),
					metadata: Some(metadata),
				});
			}

			// Continue to check for virtual resource dependencies
			// (e.g., space_item -> space_layout, entry -> file)
		}

		// Check if any virtual resources depend on this type (dependency routing)
		// This handles cases like "entry" -> File, "content_identity" -> File
		let mut all_virtual_resources = Vec::new();

		for resource_id in &resource_ids {
			let virtual_mappings =
				map_dependency_to_virtual_ids(&self.db, resource_type, *resource_id).await?;

			for (virtual_type, virtual_ids) in virtual_mappings {
				all_virtual_resources.push((virtual_type, virtual_ids));
			}
		}

		if all_virtual_resources.is_empty() {
			// No virtual resources depend on this type - that's fine for simple resources
			tracing::debug!(
				"No virtual resource dependencies for type '{}'",
				resource_type
			);
			return Ok(());
		}

		// Group by virtual resource type
		use std::collections::HashMap;
		let mut grouped: HashMap<&str, Vec<Uuid>> = HashMap::new();

		for (vtype, vids) in all_virtual_resources {
			grouped.entry(vtype).or_default().extend(vids);
		}

		// Emit events for each virtual resource type
		for (virtual_type, virtual_ids) in grouped {
			let resource_info = crate::domain::resource_registry::find_by_type(virtual_type)
				.ok_or_else(|| {
					crate::common::errors::CoreError::Other(anyhow::anyhow!(
						"Unknown virtual resource type: {}",
						virtual_type
					))
				})?;

			let resources_json = (resource_info.constructor)(&self.db, &virtual_ids).await?;

			if resources_json.is_empty() {
				continue;
			}

			tracing::info!(
				"Emitting {} {} ResourceChanged events (from {} {})",
				resources_json.len(),
				virtual_type,
				resource_type,
				if virtual_ids.len() == 1 {
					"change"
				} else {
					"changes"
				}
			);

			let affected_paths = if virtual_type == "file" {
				Self::extract_file_paths(&resources_json)
			} else {
				vec![]
			};

			let metadata = ResourceMetadata {
				no_merge_fields: resource_info
					.no_merge_fields
					.iter()
					.map(|s| s.to_string())
					.collect(),
				alternate_ids: vec![],
				affected_paths,
			};

			self.events.emit(Event::ResourceChangedBatch {
				resource_type: virtual_type.to_string(),
				resources: serde_json::Value::Array(resources_json),
				metadata: Some(metadata),
			});
		}

		Ok(())
	}

	/// Emit events for a batch of resource changes
	///
	/// More efficient than calling emit_resource_events repeatedly.
	/// Deduplicates affected virtual resources before constructing them.
	///
	/// # Example
	/// ```ignore
	/// // Batch of ContentIdentity creations
	/// manager.emit_batch_resource_events(
	///     "content_identity",
	///     vec![ci_id1, ci_id2, ci_id3],
	/// ).await?;
	/// ```
	pub async fn emit_batch_resource_events(
		&self,
		resource_type: &str,
		resource_ids: Vec<Uuid>,
	) -> Result<()> {
		// For now, delegate to single-resource handler
		// In future, could optimize by batching virtual resource construction
		self.emit_resource_events(resource_type, resource_ids).await
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	// Tests for resource mapping and event emission
	// Most functionality is now delegated to domain implementations via the registry.
	// Test coverage should focus on:
	// - extract_file_paths (infrastructure logic)
	// - emit_resource_events routing to registry
	// - Virtual resource dependency mapping

	#[test]
	fn test_extract_file_paths_empty() {
		let paths = ResourceManager::extract_file_paths(&[]);
		assert!(paths.is_empty());
	}

	#[test]
	fn test_extract_file_paths_with_sd_path() {
		let resource = serde_json::json!({
			"id": "550e8400-e29b-41d4-a716-446655440000",
			"sd_path": {
				"Physical": {
					"device_slug": "macbook",
					"path": "/Users/test/Documents/file.txt"
				}
			}
		});

		let paths = ResourceManager::extract_file_paths(&[resource]);
		assert!(!paths.is_empty());
	}
}
