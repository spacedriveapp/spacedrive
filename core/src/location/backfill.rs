//! # Volume ID Backfill Service
//!
//! `core::location::backfill` populates NULL volume_id values for existing locations
//! and their root entries. This handles legacy locations created before volume tracking
//! was fully implemented, ensuring they participate correctly in sync and change detection.
//!
//! The backfill runs at library startup and is idempotent - it only processes locations
//! that still have NULL volume_id, skipping those already resolved by lazy indexing.

use crate::{
	infra::db::entities, library::Library, ops::indexing::PathResolver,
	volume::manager::VolumeManager,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Result of a backfill operation
#[derive(Debug, Default)]
pub struct BackfillResult {
	/// Number of locations with NULL volume_id found
	pub locations_found: usize,
	/// Number of locations successfully backfilled
	pub locations_backfilled: usize,
	/// Number of locations that failed backfill (with reasons)
	pub locations_failed: Vec<(i32, String)>,
	/// Number of root entries updated
	pub entries_updated: usize,
}

impl BackfillResult {
	pub fn is_success(&self) -> bool {
		self.locations_failed.is_empty()
	}

	pub fn summary(&self) -> String {
		if self.locations_found == 0 {
			"No locations with NULL volume_id found".to_string()
		} else if self.is_success() {
			format!(
				"Backfilled {}/{} locations ({} entries updated)",
				self.locations_backfilled, self.locations_found, self.entries_updated
			)
		} else {
			format!(
				"Backfilled {}/{} locations, {} failed ({} entries updated)",
				self.locations_backfilled,
				self.locations_found,
				self.locations_failed.len(),
				self.entries_updated
			)
		}
	}
}

/// Backfill NULL volume_id values for all locations in a library.
///
/// This service queries all locations with NULL volume_id, resolves their volume
/// via VolumeManager, and persists the volume_id using the same helpers as lazy
/// indexing. It's safe to run multiple times - already-resolved locations are skipped.
///
/// Call this at library startup before any indexing or sync operations to ensure
/// all locations have valid volume_id references.
pub async fn backfill_location_volume_ids(
	library: &Library,
	volume_manager: &VolumeManager,
) -> BackfillResult {
	let mut result = BackfillResult::default();
	let db = library.db().conn();

	// Find all locations with NULL volume_id
	let locations_with_null_volume = match entities::location::Entity::find()
		.filter(entities::location::Column::VolumeId.is_null())
		.all(db)
		.await
	{
		Ok(locations) => locations,
		Err(e) => {
			warn!("Failed to query locations for volume_id backfill: {}", e);
			result
				.locations_failed
				.push((0, format!("Query failed: {}", e)));
			return result;
		}
	};

	result.locations_found = locations_with_null_volume.len();

	if locations_with_null_volume.is_empty() {
		debug!("No locations with NULL volume_id found - backfill not needed");
		return result;
	}

	info!(
		"Found {} locations with NULL volume_id, starting backfill",
		locations_with_null_volume.len()
	);

	for location in locations_with_null_volume {
		let location_id = location.id;
		let location_name = location
			.name
			.clone()
			.unwrap_or_else(|| "Unknown".to_string());

		// Skip locations without entry_id (not yet synced)
		let entry_id = match location.entry_id {
			Some(id) => id,
			None => {
				debug!(
					"Skipping location {} '{}' - no entry_id (not yet synced)",
					location_id, location_name
				);
				result
					.locations_failed
					.push((location_id, "No entry_id - not yet synced".to_string()));
				continue;
			}
		};

		// Resolve the location's path from the entry
		let location_path = match PathResolver::get_full_path(db, entry_id).await {
			Ok(path) => path,
			Err(e) => {
				warn!(
					"Failed to resolve path for location {} '{}': {}",
					location_id, location_name, e
				);
				result
					.locations_failed
					.push((location_id, format!("Path resolution failed: {}", e)));
				continue;
			}
		};

		// Find the volume for this path
		let volume = match volume_manager.volume_for_path(&location_path).await {
			Some(vol) => vol,
			None => {
				// Volume not currently mounted - will be resolved on next index when volume comes online
				debug!(
					"No volume found for location {} '{}' at {} - volume may be offline or unmounted",
					location_id,
					location_name,
					location_path.display()
				);
				result.locations_failed.push((
					location_id,
					format!(
						"Volume offline or unmounted for path: {}. Mount the volume and re-index to resolve.",
						location_path.display()
					),
				));
				continue;
			}
		};

		// Ensure volume is in database and get its ID
		let volume_id = match volume_manager.ensure_volume_in_db(&volume, library).await {
			Ok(id) => id,
			Err(e) => {
				warn!(
					"Failed to ensure volume in database for location {} '{}': {}",
					location_id, location_name, e
				);
				result
					.locations_failed
					.push((location_id, format!("Volume DB insert failed: {}", e)));
				continue;
			}
		};

		// Update location and its root entry with the volume_id
		if let Err(e) = crate::location::manager::update_location_volume_id(
			db,
			location_id,
			Some(entry_id),
			volume_id,
		)
		.await
		{
			warn!(
				"Failed to update volume_id for location {} '{}': {}",
				location_id, location_name, e
			);
			result
				.locations_failed
				.push((location_id, format!("Update failed: {}", e)));
			continue;
		}

		info!(
			"Backfilled volume_id={} for location {} '{}' at {}",
			volume_id,
			location_id,
			location_name,
			location_path.display()
		);

		result.locations_backfilled += 1;
		result.entries_updated += 1; // Root entry is updated along with location
	}

	info!("Volume ID backfill complete: {}", result.summary());
	result
}

/// Check if any locations need volume_id backfill.
///
/// This is a lightweight check that can be used to determine if backfill is needed
/// before running the full backfill operation.
pub async fn needs_backfill(library: &Library) -> Result<bool, sea_orm::DbErr> {
	use sea_orm::PaginatorTrait;

	let count = entities::location::Entity::find()
		.filter(entities::location::Column::VolumeId.is_null())
		.count(library.db().conn())
		.await?;

	Ok(count > 0)
}

/// Get count of locations needing backfill for diagnostics.
pub async fn backfill_needed_count(library: &Library) -> Result<u64, sea_orm::DbErr> {
	use sea_orm::PaginatorTrait;

	entities::location::Entity::find()
		.filter(entities::location::Column::VolumeId.is_null())
		.count(library.db().conn())
		.await
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_backfill_result_summary() {
		let mut result = BackfillResult::default();
		assert_eq!(result.summary(), "No locations with NULL volume_id found");

		result.locations_found = 5;
		result.locations_backfilled = 5;
		result.entries_updated = 5;
		assert_eq!(
			result.summary(),
			"Backfilled 5/5 locations (5 entries updated)"
		);

		result.locations_backfilled = 3;
		result.locations_failed = vec![(1, "error".to_string()), (2, "error".to_string())];
		assert_eq!(
			result.summary(),
			"Backfilled 3/5 locations, 2 failed (5 entries updated)"
		);
	}

	#[test]
	fn test_backfill_result_is_success() {
		let mut result = BackfillResult::default();
		assert!(result.is_success());

		result.locations_failed.push((1, "error".to_string()));
		assert!(!result.is_success());
	}
}
