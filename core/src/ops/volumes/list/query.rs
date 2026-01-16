//! Volume list query

use super::output::VolumeListOutput;
use crate::{
	context::CoreContext,
	infra::{
		db::entities,
		query::{LibraryQuery, QueryError, QueryResult},
	},
	volume::VolumeFingerprint,
};
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QuerySelect};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{collections::HashMap, sync::Arc};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub enum VolumeFilter {
	/// Only return tracked volumes
	TrackedOnly,
	/// Only return untracked volumes
	UntrackedOnly,
	/// Return all volumes (tracked and untracked)
	All,
}

impl Default for VolumeFilter {
	fn default() -> Self {
		Self::TrackedOnly
	}
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct VolumeListQueryInput {
	/// Filter volumes by tracking status (default: TrackedOnly)
	#[serde(default)]
	pub filter: VolumeFilter,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct VolumeListQuery {
	filter: VolumeFilter,
}

impl VolumeListQuery {
	/// Get file count from ephemeral index if this volume's mount point has been indexed
	///
	/// Returns the total number of entries under this mount point (recursive count).
	/// Only returns counts for volumes on the current device (where the ephemeral index lives).
	/// If a snapshot exists on disk but isn't loaded yet, returns None (lazy loading happens
	/// when user explicitly indexes the volume).
	fn get_ephemeral_file_count(
		index: &crate::ops::indexing::ephemeral::EphemeralIndex,
		indexed_paths: &[std::path::PathBuf],
		mount_point: &Option<String>,
		volume_device_id: Uuid,
		current_device_id: Uuid,
	) -> Option<usize> {
		// Only return file count if this volume belongs to the current device
		// (ephemeral index only exists on the local device)
		if volume_device_id != current_device_id {
			return None;
		}

		let mount_path = mount_point.as_ref()?;
		let mount_pathbuf = std::path::PathBuf::from(mount_path);

		// Check if this exact mount point is indexed in memory
		if indexed_paths.contains(&mount_pathbuf) {
			// Use efficient method to count entries under this mount point
			let count = index.count_entries_under_path(&mount_pathbuf);
			return Some(count);
		}

		// Note: Snapshots exist on disk but aren't auto-loaded to avoid blocking startup.
		// They'll be loaded when the user explicitly clicks "Index" on a volume.
		None
	}

	/// Calculate unique bytes for a volume by deduplicating content using content_identity
	///
	/// NOTE: This should NOT be called in the query path! This is expensive.
	/// Instead, the VolumeManager should periodically calculate this for volumes
	/// on the current device and update the database. The query just reads the cached value.
	///
	/// This function is kept here for reference and can be used by the volume manager.
	#[allow(dead_code)]
	async fn calculate_unique_bytes_for_volume(
		db: &sea_orm::DatabaseConnection,
		mount_point: &str,
	) -> QueryResult<Option<u64>> {
		use sea_orm::{DbBackend, FromQueryResult, Statement};

		// Query to calculate unique bytes on this volume:
		// 1. Join entries with directory_paths to get full paths
		// 2. Filter entries whose paths start with this volume's mount point
		// 3. Join with content_identity to get content hashes
		// 4. Group by content_hash to deduplicate, then sum total_size
		let query = r#"
			SELECT COALESCE(SUM(unique_size), 0) as unique_bytes
			FROM (
				SELECT ci.content_hash, ci.total_size as unique_size
				FROM entries e
				INNER JOIN directory_paths dp ON e.id = dp.entry_id
				INNER JOIN content_identities ci ON e.content_id = ci.id
				WHERE dp.path LIKE ? || '%'
				  AND e.kind = 0
				GROUP BY ci.content_hash, ci.total_size
			)
		"#;

		#[derive(FromQueryResult)]
		struct UniqueResult {
			unique_bytes: i64,
		}

		let result = UniqueResult::find_by_statement(Statement::from_sql_and_values(
			DbBackend::Sqlite,
			query,
			vec![mount_point.to_string().into()],
		))
		.one(db)
		.await?;

		match result {
			Some(r) if r.unique_bytes > 0 => Ok(Some(r.unique_bytes as u64)),
			_ => Ok(None),
		}
	}

}

impl LibraryQuery for VolumeListQuery {
	type Input = VolumeListQueryInput;
	type Output = VolumeListOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self {
			filter: input.filter,
		})
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let library_id = session
			.current_library_id
			.ok_or_else(|| QueryError::Internal("No library selected".to_string()))?;

		let library = context
			.libraries()
			.await
			.get_library(library_id)
			.await
			.ok_or_else(|| QueryError::Internal("Library not found".to_string()))?;

		let db = library.db().conn();

		// Get tracked volumes from database (includes volumes from ALL devices)
		// Only include user-visible volumes
		let tracked_volumes = entities::volume::Entity::find()
			.filter(entities::volume::Column::IsUserVisible.eq(true))
			.all(db)
			.await?;

		tracing::info!(
			count = tracked_volumes.len(),
			filter = ?self.filter,
			"[volumes.list] Fetched tracked volumes from database"
		);

		// Fetch all devices to get slugs
		let devices = entities::device::Entity::find().all(db).await?;
		let device_slug_map: HashMap<Uuid, String> =
			devices.into_iter().map(|d| (d.uuid, d.slug)).collect();

		// Create a map of tracked volumes by fingerprint
		let mut tracked_map: HashMap<String, entities::volume::Model> = tracked_volumes
			.into_iter()
			.map(|v| (v.fingerprint.clone(), v))
			.collect();

		tracing::info!(
			tracked_map_size = tracked_map.len(),
			"[volumes.list] Created tracked_map"
		);

		let volume_manager = &context.volume_manager;
		let mut volumes = Vec::new();

		// Get current device ID
		let current_device_id = context
			.device_manager
			.device_id()
			.unwrap_or_else(|_| Uuid::nil());

		// Get live volumes from VolumeManager (current device)
		let live_volumes = volume_manager.get_all_volumes().await;
		let mut live_volumes_map: HashMap<String, crate::domain::volume::Volume> = live_volumes
			.into_iter()
			.map(|v| (v.fingerprint.0.clone(), v))
			.collect();

		match self.filter {
			VolumeFilter::TrackedOnly | VolumeFilter::All => {
				// For tracked volumes, prefer live data if available, otherwise use DB
				for tracked_vol in tracked_map.values() {
					if let Some(live_vol) = live_volumes_map.remove(&tracked_vol.fingerprint) {
						// Use live volume data (current device, online)
						volumes.push(live_vol);
					} else {
					// Volume is offline or on another device
					// Skip offline volumes from current device to avoid duplicates
					if tracked_vol.device_id == current_device_id && !tracked_vol.is_online {
						continue;
					}
					volumes.push(tracked_vol.to_tracked_volume().to_offline_volume());
						volumes.push(tracked_vol.to_tracked_volume().to_offline_volume());
					}
				}

				// For All filter, also add untracked volumes
				if matches!(self.filter, VolumeFilter::All) {
					// Add remaining live volumes that aren't tracked
					for vol in live_volumes_map.into_values() {
						if vol.is_user_visible {
							volumes.push(vol);
						}
					}
				}
			}
			VolumeFilter::UntrackedOnly => {
				// Only return untracked volumes from volume manager
				for vol in live_volumes_map.into_values() {
					if !vol.is_tracked && vol.is_user_visible {
						volumes.push(vol);
					}
				}
			}
		}

		tracing::info!(
			volume_count = volumes.len(),
			filter = ?self.filter,
			"[volumes.list] Returning volumes"
		);

		Ok(VolumeListOutput { volumes })
	}
}

crate::register_library_query!(VolumeListQuery, "volumes.list");
