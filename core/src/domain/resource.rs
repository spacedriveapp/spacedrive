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
}

/// Helper to check if a resource type is virtual (has dependencies)
pub fn is_virtual_resource(resource_type: &str) -> bool {
	match resource_type {
		"file" => true, // File = Entry + ContentIdentity + Sidecar
		_ => false,
	}
}

/// Get dependencies for a resource type
pub fn get_dependencies(resource_type: &str) -> &'static [&'static str] {
	match resource_type {
		"file" => &["entry", "content_identity", "sidecar"],
		_ => &[],
	}
}

/// Map a dependency change to affected virtual resource IDs
///
/// This is the core mapping function used by the resource manager.
/// Given a change to a dependency (e.g., ContentIdentity created),
/// determine which virtual resources (e.g., Files) are affected.
///
/// Returns: (virtual_resource_type, vec![virtual_resource_ids])
pub async fn map_dependency_to_virtual_ids(
	db: &sea_orm::DatabaseConnection,
	dependency_type: &str,
	dependency_id: Uuid,
) -> crate::common::errors::Result<Vec<(&'static str, Vec<Uuid>)>> {
	use crate::infra::db::entities::{content_identity, entry, sidecar};
	use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

	let mut results = Vec::new();

	// Check if any virtual resources depend on this type
	match dependency_type {
		"entry" => {
			// File depends on Entry
			// File ID = Entry UUID
			results.push(("file", vec![dependency_id]));
		}

		"content_identity" => {
			// File depends on ContentIdentity
			// Find all Entries with this content_id
			let ci = content_identity::Entity::find()
				.filter(content_identity::Column::Uuid.eq(dependency_id))
				.one(db)
				.await?
				.ok_or_else(|| crate::common::errors::CoreError::NotFound(format!("ContentIdentity {} not found", dependency_id)))?;

			let entry_models = entry::Entity::find()
				.filter(entry::Column::ContentId.eq(ci.id))
				.all(db)
				.await?;

			let file_ids: Vec<Uuid> = entry_models
				.into_iter()
				.filter_map(|e| e.uuid)
				.collect();

			if !file_ids.is_empty() {
				results.push(("file", file_ids));
			}
		}

		"sidecar" => {
			// File depends on Sidecar
			// Find Entry by content_uuid
			let sc = sidecar::Entity::find()
				.filter(sidecar::Column::Uuid.eq(dependency_id))
				.one(db)
				.await?
				.ok_or_else(|| crate::common::errors::CoreError::NotFound(format!("Sidecar {} not found", dependency_id)))?;

			// Find entries with matching content_identity UUID
			let ci_opt = content_identity::Entity::find()
				.filter(content_identity::Column::Uuid.eq(sc.content_uuid))
				.one(db)
				.await?;

			if let Some(ci) = ci_opt {
				let entry_models = entry::Entity::find()
					.filter(entry::Column::ContentId.eq(ci.id))
					.all(db)
					.await?;

				let file_ids: Vec<Uuid> = entry_models
					.into_iter()
					.filter_map(|e| e.uuid)
					.collect();

				if !file_ids.is_empty() {
					results.push(("file", file_ids));
				}
			}
		}

		_ => {
			// No virtual resources depend on this type
		}
	}

	Ok(results)
}
