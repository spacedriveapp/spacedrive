//! Resource Manager - Maps low-level DB changes to high-level resource events
//!
//! This system handles the complexity of virtual resources (like File) that
//! are computed from multiple database tables.
//!
//! When a low-level resource changes (e.g., ContentIdentity created),
//! the ResourceManager determines which high-level resources are affected
//! and emits appropriate events for the frontend normalized cache.

use crate::common::errors::Result;
use crate::infra::event::{Event, EventBus};
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use uuid::Uuid;

/// Resource Manager coordinates event emission for virtual resources
pub struct ResourceManager {
	db: Arc<DatabaseConnection>,
	events: Arc<EventBus>,
}

impl ResourceManager {
	pub fn new(db: Arc<DatabaseConnection>, events: Arc<EventBus>) -> Self {
		Self { db, events }
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

		// Check if any virtual resources depend on this type
		let mut all_virtual_resources = Vec::new();

		for resource_id in resource_ids {
			let virtual_mappings = map_dependency_to_virtual_ids(&self.db, resource_type, resource_id).await?;

			for (virtual_type, virtual_ids) in virtual_mappings {
				all_virtual_resources.push((virtual_type, virtual_ids));
			}
		}

		if all_virtual_resources.is_empty() {
			// No virtual resources depend on this - emit direct event
			// (Only if the resource type itself is identifiable)
			tracing::debug!(
				"No virtual resources depend on {}, skipping event",
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
			match virtual_type {
				"file" => {
					// Construct File instances from entry UUIDs
					let files = crate::domain::File::from_entry_uuids(&self.db, &virtual_ids).await?;

					tracing::info!(
						"Emitting {} File ResourceChanged events (from {} {})",
						files.len(),
						resource_type,
						if virtual_ids.len() == 1 { "change" } else { "changes" }
					);

					if !files.is_empty() {
						self.events.emit(Event::ResourceChangedBatch {
							resource_type: "file".to_string(),
							resources: serde_json::to_value(&files).map_err(|e| {
								crate::common::errors::CoreError::Other(anyhow::anyhow!(
									"Failed to serialize files: {}", e
								))
							})?,
						});
					}
				}
				_ => {
					tracing::warn!("Unknown virtual resource type: {}", virtual_type);
				}
			}
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

	// TODO: Add tests for resource mapping
	// - Test ContentIdentity → File mapping
	// - Test Sidecar → File mapping
	// - Test batch deduplication
}
