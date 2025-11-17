//! Resource abstraction for normalized cache and event system
//!
//! This module provides traits and utilities for managing resources across
//! the sync system, event emission, and frontend normalized cache.

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
