//! Resource Manager - Maps low-level DB changes to high-level resource events
//!
//! This system handles the complexity of virtual resources (like File) that
//! are computed from multiple database tables.
//!
//! When a low-level resource changes (e.g., ContentIdentity created),
//! the ResourceManager determines which high-level resources are affected
//! and emits appropriate events for the frontend normalized cache.

use crate::common::errors::Result;
use crate::domain::resource::Identifiable;
use crate::infra::event::{Event, EventBus, ResourceMetadata};
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

	/// Build a File object from an entry model (for space item resolution)
	async fn build_file_from_entry(
		entry_model: crate::infra::db::entities::entry::Model,
		item_type: &crate::domain::ItemType,
		db: &DatabaseConnection,
	) -> Option<crate::domain::File> {
		use crate::domain::{ContentIdentity, ContentKind, File, ItemType, SdPath, Sidecar};
		use crate::infra::db::entities::{content_identity, sidecar};
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

		let sd_path = match item_type {
			ItemType::Path { sd_path } => sd_path.clone(),
			_ => return None,
		};

		let content_identity = if let Some(content_id) = entry_model.content_id {
			content_identity::Entity::find_by_id(content_id)
				.one(db)
				.await
				.ok()
				.flatten()
				.map(|ci| ContentIdentity {
					uuid: ci.uuid.unwrap_or_else(Uuid::new_v4),
					kind: ContentKind::from_id(ci.kind_id),
					content_hash: ci.content_hash,
					integrity_hash: ci.integrity_hash,
					mime_type_id: ci.mime_type_id,
					text_content: ci.text_content,
					total_size: ci.total_size,
					entry_count: ci.entry_count,
					first_seen_at: ci.first_seen_at,
					last_verified_at: ci.last_verified_at,
				})
		} else {
			None
		};

		let sidecars = if let Some(ref ci) = content_identity {
			if let Some(uuid) = Some(ci.uuid) {
				sidecar::Entity::find()
					.filter(sidecar::Column::ContentUuid.eq(uuid))
					.all(db)
					.await
					.ok()
					.unwrap_or_default()
					.into_iter()
					.map(|s| Sidecar {
						id: s.id,
						content_uuid: s.content_uuid,
						kind: s.kind,
						variant: s.variant,
						format: s.format,
						status: s.status,
						size: s.size,
						created_at: s.created_at,
						updated_at: s.updated_at,
					})
					.collect()
			} else {
				Vec::new()
			}
		} else {
			Vec::new()
		};

		let mut file = File::from_entity_model(entry_model, sd_path);
		file.content_identity = content_identity;
		file.sidecars = sidecars;
		if let Some(ref ci) = file.content_identity {
			file.content_kind = ci.kind;
		}

		Some(file)
	}

	/// Emit direct ResourceChanged events for simple resources
	async fn emit_direct_events(&self, resource_type: &str, resource_ids: &[Uuid]) -> Result<()> {
		use crate::domain::{GroupType, ItemType, Space, SpaceGroup, SpaceItem};
		use crate::infra::db::entities::{space, space_group, space_item};
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

		match resource_type {
			"space" => {
				for &space_id in resource_ids {
					if let Some(space_model) = space::Entity::find()
						.filter(space::Column::Uuid.eq(space_id))
						.one(&*self.db)
						.await?
					{
						let space = Space {
							id: space_model.uuid,
							name: space_model.name,
							icon: space_model.icon,
							color: space_model.color,
							order: space_model.order,
							created_at: space_model.created_at.into(),
							updated_at: space_model.updated_at.into(),
						};

						self.events.emit(Event::ResourceChanged {
							resource_type: "space".to_string(),
							resource: serde_json::to_value(&space).map_err(|e| {
								crate::common::errors::CoreError::Other(anyhow::anyhow!(
									"Failed to serialize space: {}",
									e
								))
							})?,
							metadata: None,
						});
					}
				}
			}
			"space_group" => {
				for &group_id in resource_ids {
					if let Some(group_model) = space_group::Entity::find()
						.filter(space_group::Column::Uuid.eq(group_id))
						.one(&*self.db)
						.await?
					{
						let space_model = space::Entity::find_by_id(group_model.space_id)
							.one(&*self.db)
							.await?;

						let space_id = space_model.map(|s| s.uuid).unwrap_or(group_id);

						let group_type: GroupType = serde_json::from_str(&group_model.group_type)
							.map_err(|e| {
							crate::common::errors::CoreError::Other(anyhow::anyhow!(
								"Failed to parse group_type: {}",
								e
							))
						})?;

						let group = SpaceGroup {
							id: group_model.uuid,
							space_id,
							name: group_model.name,
							group_type,
							is_collapsed: group_model.is_collapsed,
							order: group_model.order,
							created_at: group_model.created_at.into(),
						};

						self.events.emit(Event::ResourceChanged {
							resource_type: "space_group".to_string(),
							resource: serde_json::to_value(&group).map_err(|e| {
								crate::common::errors::CoreError::Other(anyhow::anyhow!(
									"Failed to serialize group: {}",
									e
								))
							})?,
							metadata: None,
						});
					}
				}
			}
			"location" => {
				use crate::domain::addressing::SdPath;
				use crate::infra::db::entities::{device, directory_paths, entry, location};
				use crate::ops::locations::list::output::LocationInfo;

				for &location_id in resource_ids {
					// Build LocationInfo the same way as LocationsListQuery
					let location_with_entry = location::Entity::find()
						.filter(location::Column::Uuid.eq(location_id))
						.find_also_related(entry::Entity)
						.one(&*self.db)
						.await?;

					if let Some((loc, entry_opt)) = location_with_entry {
						let Some(entry) = entry_opt else {
							tracing::warn!(
								"Location {} has no root entry, skipping event",
								location_id
							);
							continue;
						};

						let Some(dir_path) = directory_paths::Entity::find_by_id(entry.id)
							.one(&*self.db)
							.await?
						else {
							tracing::warn!(
								"No directory path for location {} entry {}",
								location_id,
								entry.id
							);
							continue;
						};

						let Some(device_model) = device::Entity::find_by_id(loc.device_id)
							.one(&*self.db)
							.await?
						else {
							tracing::warn!("Device not found for location {}", location_id);
							continue;
						};

						let sd_path = SdPath::Physical {
							device_slug: device_model.slug.clone(),
							path: dir_path.path.clone().into(),
						};

						let job_policies = loc
							.job_policies
							.as_ref()
							.and_then(|json| serde_json::from_str(json).ok())
							.unwrap_or_default();

						let location_info = LocationInfo {
							id: loc.uuid,
							path: dir_path.path.into(),
							name: loc.name.clone(),
							sd_path,
							job_policies,
							index_mode: loc.index_mode.clone(),
							scan_state: loc.scan_state.clone(),
							last_scan_at: loc.last_scan_at,
							error_message: loc.error_message.clone(),
							total_file_count: loc.total_file_count,
							total_byte_size: loc.total_byte_size,
							created_at: loc.created_at,
							updated_at: loc.updated_at,
						};

						self.events.emit(Event::ResourceChanged {
							resource_type: "location".to_string(),
							resource: serde_json::to_value(&location_info).map_err(|e| {
								crate::common::errors::CoreError::Other(anyhow::anyhow!(
									"Failed to serialize location: {}",
									e
								))
							})?,
							metadata: None,
						});
					}
				}
			}
			"space_item" => {
				for &item_id in resource_ids {
					if let Some(item_model) = space_item::Entity::find()
						.filter(space_item::Column::Uuid.eq(item_id))
						.one(&*self.db)
						.await?
					{
						let space_model = space::Entity::find_by_id(item_model.space_id)
							.one(&*self.db)
							.await?;

						let space_id = space_model.map(|s| s.uuid).unwrap_or(item_id);

						let item_type: ItemType = serde_json::from_str(&item_model.item_type)
							.map_err(|e| {
								crate::common::errors::CoreError::Other(anyhow::anyhow!(
									"Failed to parse item_type: {}",
									e
								))
							})?;

						let resolved_file = if let Some(entry_id) = item_model.entry_id {
							use crate::infra::db::entities::entry;
							if let Some(entry_model) =
								entry::Entity::find_by_id(entry_id).one(&*self.db).await?
							{
								Self::build_file_from_entry(entry_model, &item_type, &self.db)
									.await
									.map(Box::new)
							} else {
								None
							}
						} else {
							None
						};

						let item = SpaceItem {
							id: item_model.uuid,
							space_id,
							group_id: item_model.group_id.and_then(|id| {
								// Need to look up group UUID from ID
								// For now just skip group_id
								None
							}),
							item_type,
							order: item_model.order,
							created_at: item_model.created_at.into(),
							resolved_file,
						};

						self.events.emit(Event::ResourceChanged {
							resource_type: "space_item".to_string(),
							resource: serde_json::to_value(&item).map_err(|e| {
								crate::common::errors::CoreError::Other(anyhow::anyhow!(
									"Failed to serialize item: {}",
									e
								))
							})?,
							metadata: None,
						});
					}
				}
			}
			_ => {
				// Unknown resource type, skip direct emission
			}
		}

		Ok(())
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

		// Emit direct events first (for simple list queries)
		self.emit_direct_events(resource_type, &resource_ids)
			.await?;

		// Check if resource_type IS a registered virtual resource
		// If so, emit directly using its constructor (e.g., "file" with entry UUIDs)
		if let Some(resource_info) = crate::domain::resource_registry::find_by_type(resource_type) {
			let resources_json = (resource_info.constructor)(&self.db, &resource_ids).await?;

			if !resources_json.is_empty() {
				tracing::info!(
					"Emitting {} {} ResourceChanged events (direct virtual resource)",
					resources_json.len(),
					resource_type
				);

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

			return Ok(());
		}

		// Check if any virtual resources depend on this type
		let mut all_virtual_resources = Vec::new();

		for resource_id in &resource_ids {
			let virtual_mappings =
				map_dependency_to_virtual_ids(&self.db, resource_type, *resource_id).await?;

			for (virtual_type, virtual_ids) in virtual_mappings {
				all_virtual_resources.push((virtual_type, virtual_ids));
			}
		}

		if all_virtual_resources.is_empty() {
			return Ok(());
		}

		// Group by virtual resource type
		use std::collections::HashMap;
		let mut grouped: HashMap<&str, Vec<Uuid>> = HashMap::new();

		for (vtype, vids) in all_virtual_resources {
			grouped.entry(vtype).or_default().extend(vids);
		}

		// Emit events for each virtual resource type (now fully generic!)
		for (virtual_type, virtual_ids) in grouped {
			// Find the resource info from the registry
			let resource_info = crate::domain::resource_registry::find_by_type(virtual_type)
				.ok_or_else(|| {
					crate::common::errors::CoreError::Other(anyhow::anyhow!(
						"Unknown virtual resource type: {}",
						virtual_type
					))
				})?;

			// Call the constructor to build virtual resources
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

			// Extract affected paths for path-scoped filtering
			let affected_paths = if virtual_type == "file" {
				Self::extract_file_paths(&resources_json)
			} else {
				vec![]
			};

			// Build metadata
			let metadata = ResourceMetadata {
				no_merge_fields: resource_info
					.no_merge_fields
					.iter()
					.map(|s| s.to_string())
					.collect(),
				// Note: alternate_ids would need to be extracted from deserialized resources
				// For now, we'll leave it empty as it's harder to extract generically
				alternate_ids: vec![],
				affected_paths,
			};

			// Emit as batch for efficiency
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

	// TODO: Add tests for resource mapping
	// - Test ContentIdentity → File mapping
	// - Test Sidecar → File mapping
	// - Test batch deduplication
}
