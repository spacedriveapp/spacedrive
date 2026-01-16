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

	/// Infer disk type from device model or volume type
	/// TODO: Implement this properly!!! - jamie
	fn infer_disk_type(
		device_model: &Option<String>,
		volume_type: &Option<String>,
	) -> Option<String> {
		// Check device model first
		if let Some(model) = device_model {
			let model_lower = model.to_lowercase();
			if model_lower.contains("ssd") || model_lower.contains("nvme") {
				return Some("SSD".to_string());
			}
			if model_lower.contains("hdd") || model_lower.contains("hard") {
				return Some("HDD".to_string());
			}
		}

		// Check volume type
		if let Some(vtype) = volume_type {
			let vtype_lower = vtype.to_lowercase();
			if vtype_lower.contains("ssd") {
				return Some("SSD".to_string());
			}
			if vtype_lower.contains("external") {
				return Some("External".to_string());
			}
			if vtype_lower.contains("network") || vtype_lower.contains("cloud") {
				return Some("Network".to_string());
			}
		}

		None
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
		let mut volume_items = Vec::new();

		// Get ephemeral cache to check for indexed file counts
		let ephemeral_cache = context.ephemeral_cache();
		let indexed_paths = ephemeral_cache.indexed_paths();
		let global_index = ephemeral_cache.get_global_index();
		let index = global_index.read().await;

		// Get current device ID to filter ephemeral index results
		let current_device_id = context
			.device_manager
			.device_id()
			.unwrap_or_else(|_| Uuid::nil());

		match self.filter {
			VolumeFilter::TrackedOnly | VolumeFilter::All => {
				// For TrackedOnly and All, return volumes from database (all devices)
				for tracked_vol in tracked_map.values() {
					// Read cached unique_bytes from database (calculated by volume manager)
					let unique_bytes = tracked_vol.unique_bytes.map(|b| b as u64);

					// Determine disk type from device_model or volume_type
					let disk_type =
						Self::infer_disk_type(&tracked_vol.device_model, &tracked_vol.volume_type);

					// Get device slug for this volume
					let device_slug = device_slug_map
						.get(&tracked_vol.device_id)
						.cloned()
						.unwrap_or_else(|| "unknown".to_string());

					// Get file count from ephemeral index if available
					let total_file_count = Self::get_ephemeral_file_count(
						&index,
						&indexed_paths,
						&tracked_vol.mount_point,
						tracked_vol.device_id,
						current_device_id,
					);

					volume_items.push(super::output::VolumeItem {
						id: tracked_vol.uuid,
						name: tracked_vol
							.display_name
							.clone()
							.unwrap_or_else(|| "Unnamed".to_string()),
						fingerprint: VolumeFingerprint(tracked_vol.fingerprint.clone()),
						volume_type: tracked_vol
							.volume_type
							.clone()
							.unwrap_or_else(|| "Unknown".to_string()),
						mount_point: tracked_vol.mount_point.clone(),
						is_tracked: true,
						is_online: tracked_vol.is_online,
						total_capacity: tracked_vol.total_capacity.map(|c| c as u64),
						available_capacity: tracked_vol.available_capacity.map(|c| c as u64),
						unique_bytes,
						file_system: tracked_vol.file_system.clone(),
						disk_type,
						read_speed_mbps: tracked_vol.read_speed_mbps.map(|s| s as u32),
						write_speed_mbps: tracked_vol.write_speed_mbps.map(|s| s as u32),
						device_id: tracked_vol.device_id,
						device_slug,
						total_file_count,
					});
				}

				// For All filter, also add untracked volumes from volume_manager
				if matches!(self.filter, VolumeFilter::All) {
					let all_volumes = volume_manager.get_all_volumes().await;
					for vol in all_volumes {
						// Only show user-visible volumes
						if !tracked_map.contains_key(&vol.fingerprint.0) && vol.is_user_visible {
							let device_slug = device_slug_map
								.get(&vol.device_id)
								.cloned()
								.unwrap_or_else(|| "unknown".to_string());

							let mount_point_str = Some(vol.mount_point.to_string_lossy().to_string());

							// Get file count from ephemeral index if available
							let total_file_count = Self::get_ephemeral_file_count(
								&index,
								&indexed_paths,
								&mount_point_str,
								vol.device_id,
								current_device_id,
							);

							volume_items.push(super::output::VolumeItem {
								id: vol.id,
								name: vol.display_name.clone().unwrap_or_else(|| vol.name.clone()),
								fingerprint: vol.fingerprint.clone(),
								volume_type: format!("{:?}", vol.volume_type),
								mount_point: mount_point_str,
								is_tracked: false,
								is_online: vol.is_mounted,
								total_capacity: Some(vol.total_capacity),
								available_capacity: Some(vol.available_space),
								unique_bytes: None,
								file_system: Some(vol.file_system.to_string()),
								disk_type: Some(format!("{:?}", vol.disk_type)),
								read_speed_mbps: vol.read_speed_mbps.map(|s| s as u32),
								write_speed_mbps: vol.write_speed_mbps.map(|s| s as u32),
								device_id: vol.device_id,
								device_slug,
								total_file_count,
							});
						}
					}
				}
			}
			VolumeFilter::UntrackedOnly => {
				// Get all detected volumes from volume manager (current device only)
				let all_volumes = volume_manager.get_all_volumes().await;

				// Only return volumes that are NOT tracked and are user-visible
				for vol in all_volumes {
					if !tracked_map.contains_key(&vol.fingerprint.0) && vol.is_user_visible {
						let device_slug = device_slug_map
							.get(&vol.device_id)
							.cloned()
							.unwrap_or_else(|| "unknown".to_string());

						let mount_point_str = Some(vol.mount_point.to_string_lossy().to_string());

						// Get file count from ephemeral index if available
						let total_file_count = Self::get_ephemeral_file_count(
							&index,
							&indexed_paths,
							&mount_point_str,
							vol.device_id,
							current_device_id,
						);

						volume_items.push(super::output::VolumeItem {
							id: vol.id,
							name: vol.display_name.clone().unwrap_or_else(|| vol.name.clone()),
							fingerprint: vol.fingerprint.clone(),
							volume_type: format!("{:?}", vol.volume_type),
							mount_point: mount_point_str,
							is_tracked: false,
							is_online: vol.is_mounted,
							total_capacity: Some(vol.total_capacity),
							available_capacity: Some(vol.available_space),
							unique_bytes: None,
							file_system: Some(vol.file_system.to_string()),
							disk_type: Some(format!("{:?}", vol.disk_type)),
							read_speed_mbps: vol.read_speed_mbps.map(|s| s as u32),
							write_speed_mbps: vol.write_speed_mbps.map(|s| s as u32),
							device_id: vol.device_id,
							device_slug,
							total_file_count,
						});
					}
				}
			}
		}

		tracing::info!(
			volume_items_count = volume_items.len(),
			filter = ?self.filter,
			"[volumes.list] Returning volume items"
		);

		Ok(VolumeListOutput {
			volumes: volume_items,
		})
	}
}

crate::register_library_query!(VolumeListQuery, "volumes.list");
