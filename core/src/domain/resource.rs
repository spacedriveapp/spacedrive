//! Resource abstraction for normalized cache and event system
//!
//! This module provides traits and utilities for managing resources across
//! the sync system, event emission, and frontend normalized cache.
//!
//! ## Architecture
//!
//! The resource system uses a trait-based pattern where each domain model
//! owns its own logic for:
//! - Identification (via `id()` and `resource_type()`)
//! - Construction (via `from_ids()`)
//! - Event emission (via `EventEmitter` auto-trait)
//! - Virtual resource routing (via `route_from_dependency()`)
//!
//! ## Resource Types
//!
//! ### Simple Resources
//! Backed by a single database table. Examples:
//! - `Space` - workspace/project
//! - `SpaceGroup` - group within a space
//! - `Location` - indexed filesystem location
//!
//! Simple resources:
//! - Implement `from_ids()` to query and construct themselves
//! - Have no `sync_dependencies()` (empty slice)
//! - Don't implement `route_from_dependency()` (default no-op)
//!
//! ### Virtual Resources
//! Computed from multiple database tables. Examples:
//! - `File` - aggregates Entry + ContentIdentity + Sidecars + Tags
//! - `SpaceLayout` - aggregates Space + Groups + Items
//!
//! Virtual resources:
//! - Implement `from_ids()` with complex joins
//! - Declare `sync_dependencies()`
//! - Implement `route_from_dependency()` to map dependency changes to affected IDs
//!
//! ## Adding a New Resource
//!
//! 1. Implement `Identifiable` trait on your domain model
//! 2. Register in `resource_registry.rs`
//! 3. Use `EventEmitter` trait for event emission
//!
//! No changes to `ResourceManager` needed!

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

/// Trait for resources that can be identified and synced across devices
///
/// Any domain model that needs to:
/// 1. Emit ResourceChanged events
/// 2. Be cached in frontend normalized cache
/// 3. Sync via Transaction Manager
///
/// Must implement this trait.
pub trait Identifiable: Serialize + for<'de> Deserialize<'de> + Type {
	/// Unique identifier for this resource instance
	fn id(&self) -> Uuid;

	/// Resource type identifier (e.g., "location", "tag", "file")
	///
	/// Must match the `resourceType` string used in frontend's `useNormalizedCache`
	fn resource_type() -> &'static str
	where
		Self: Sized;

	/// Resource types this virtual resource depends on
	///
	/// For simple resources (backed by single table), return empty slice.
	/// For virtual resources (computed from multiple tables), list dependencies.
	///
	/// Example:
	/// - Location: `&[]` (simple, backed by `locations` table)
	/// - File: `&["entry", "content_identity", "sidecar"]` (virtual)
	///
	/// The resource mapper uses this to determine which virtual resources
	/// need to be rebuilt when a dependent resource changes.
	fn sync_dependencies() -> &'static [&'static str]
	where
		Self: Sized,
	{
		&[]
	}

	/// Additional identifiers for matching updates (besides primary ID)
	///
	/// For resources with multiple valid identifiers, return alternate IDs.
	/// The frontend cache will match updates by primary ID OR any alternate ID.
	///
	/// Example:
	/// - File with content: `vec![content_identity.uuid]`
	/// - Entry with inode: `vec![inode_based_id]`
	///
	/// Default: no alternate IDs
	fn alternate_ids(&self) -> Vec<Uuid> {
		vec![]
	}

	/// Fields that should never be deep-merged, only replaced
	///
	/// For fields where merging doesn't make sense (e.g., paths, enums),
	/// list them here and they'll always be replaced with incoming value.
	///
	/// Example:
	/// - File: `&["sd_path"]` (paths are atomic, can't merge Physical + Content)
	///
	/// Default: merge all fields
	fn no_merge_fields() -> &'static [&'static str]
	where
		Self: Sized,
	{
		&[]
	}

	/// Route a dependency change to affected virtual resource IDs
	///
	/// For virtual resources, implement this to map changes in dependency resources
	/// to the IDs of affected virtual resource instances.
	///
	/// Example:
	/// - File: When ContentIdentity changes, find all Entry UUIDs that reference it
	/// - SpaceLayout: When SpaceGroup changes, find parent Space UUID
	///
	/// Default: empty (not a virtual resource, or doesn't depend on this type)
	async fn route_from_dependency(
		_db: &sea_orm::DatabaseConnection,
		_dependency_type: &str,
		_dependency_id: Uuid,
	) -> crate::common::errors::Result<Vec<Uuid>>
	where
		Self: Sized,
	{
		Ok(vec![])
	}

	/// Construct virtual resource instances from IDs
	///
	/// For virtual resources, implement this to build complete domain models
	/// from a list of IDs. This is used by ResourceManager to emit events.
	///
	/// Example:
	/// - File::from_ids() → File::from_entry_uuids()
	/// - SpaceLayout::from_ids() → SpaceLayout::from_space_ids()
	///
	/// Default: Not implemented (only virtual resources need this)
	async fn from_ids(
		_db: &sea_orm::DatabaseConnection,
		_ids: &[Uuid],
	) -> crate::common::errors::Result<Vec<Self>>
	where
		Self: Sized,
	{
		Err(crate::common::errors::CoreError::InvalidOperation(format!(
			"from_ids not implemented for {}",
			Self::resource_type()
		)))
	}
}

/// Helper trait for emitting resource events
///
/// Automatically implemented for all Identifiable resources.
/// Provides ergonomic methods for event emission without duplicating metadata logic.
pub trait EventEmitter: Identifiable {
	/// Emit a ResourceChanged event for this single resource
	fn emit_changed(
		&self,
		events: &crate::infra::event::EventBus,
	) -> crate::common::errors::Result<()> {
		let resource = serde_json::to_value(self).map_err(|e| {
			crate::common::errors::CoreError::Other(anyhow::anyhow!(
				"Failed to serialize {}: {}",
				Self::resource_type(),
				e
			))
		})?;

		events.emit(crate::infra::event::Event::ResourceChanged {
			resource_type: Self::resource_type().to_string(),
			resource,
			metadata: Some(crate::infra::event::ResourceMetadata {
				no_merge_fields: Self::no_merge_fields()
					.iter()
					.map(|s| s.to_string())
					.collect(),
				alternate_ids: self.alternate_ids(),
				affected_paths: vec![],
			}),
		});

		Ok(())
	}

	/// Emit a ResourceDeleted event for this resource
	fn emit_deleted(id: Uuid, events: &crate::infra::event::EventBus)
	where
		Self: Sized,
	{
		events.emit(crate::infra::event::Event::ResourceDeleted {
			resource_type: Self::resource_type().to_string(),
			resource_id: id,
		});
	}

	/// Emit a ResourceChangedBatch event for multiple resources
	async fn emit_changed_batch(
		db: &sea_orm::DatabaseConnection,
		events: &crate::infra::event::EventBus,
		ids: &[Uuid],
	) -> crate::common::errors::Result<()>
	where
		Self: Sized,
	{
		if ids.is_empty() {
			return Ok(());
		}

		let resources = Self::from_ids(db, ids).await?;
		let resources_json: Vec<serde_json::Value> = resources
			.iter()
			.map(|r| serde_json::to_value(r))
			.collect::<Result<Vec<_>, _>>()
			.map_err(|e| {
				crate::common::errors::CoreError::Other(anyhow::anyhow!(
					"Failed to serialize {}: {}",
					Self::resource_type(),
					e
				))
			})?;

		if resources_json.is_empty() {
			return Ok(());
		}

		events.emit(crate::infra::event::Event::ResourceChangedBatch {
			resource_type: Self::resource_type().to_string(),
			resources: serde_json::Value::Array(resources_json),
			metadata: Some(crate::infra::event::ResourceMetadata {
				no_merge_fields: Self::no_merge_fields()
					.iter()
					.map(|s| s.to_string())
					.collect(),
				alternate_ids: vec![], // Batch events don't need alternate_ids
				affected_paths: vec![],
			}),
		});

		Ok(())
	}
}

// Auto-implement EventEmitter for all Identifiable types
impl<T: Identifiable> EventEmitter for T {}

/// Map a dependency change to affected virtual resource IDs
///
/// This is the core mapping function used by the resource manager.
/// Given a change to a dependency (e.g., ContentIdentity created),
/// determine which virtual resources (e.g., Files) are affected.
///
/// Now fully generic - uses the resource registry instead of hardcoded match statements.
///
/// Returns: (virtual_resource_type, vec![virtual_resource_ids])
pub async fn map_dependency_to_virtual_ids(
	db: &sea_orm::DatabaseConnection,
	dependency_type: &str,
	dependency_id: Uuid,
) -> crate::common::errors::Result<Vec<(&'static str, Vec<Uuid>)>> {
	let mut results = Vec::new();

	// Find all virtual resources that depend on this dependency type
	let dependents = crate::domain::resource_registry::find_dependents(dependency_type);

	// For each dependent virtual resource, call its routing function
	for resource_info in dependents {
		let ids = (resource_info.router)(db, dependency_type, dependency_id).await?;

		if !ids.is_empty() {
			results.push((resource_info.resource_type, ids));
		}
	}

	Ok(results)
}
