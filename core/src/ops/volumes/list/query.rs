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
					if let Some(mut live_vol) = live_volumes_map.remove(&tracked_vol.fingerprint) {
						// Use live volume data (current device, online)
						// Mark as tracked since it's in the database
						// Use stable DB UUID to ensure consistency with ResourceChanged events
						live_vol.id = tracked_vol.uuid;
						live_vol.is_tracked = true;
						live_vol.library_id = Some(library_id);
						volumes.push(live_vol);
					} else {
						// Volume is offline or on another device
						// Skip offline volumes from current device to avoid duplicates
						if tracked_vol.device_id == current_device_id && !tracked_vol.is_online {
							continue;
						}
						let mut offline_vol = tracked_vol.to_tracked_volume().to_offline_volume();
						// Re-apply current platform visibility rules so stale DB
						// entries from earlier versions (which tracked everything)
						// inherit newly-added filters without a data migration.
						if crate::volume::utils::should_hide_by_mount_path(&offline_vol.mount_point)
						{
							offline_vol.is_user_visible = false;
							offline_vol.auto_track_eligible = false;
						}
						volumes.push(offline_vol);
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

		// Paired devices' volumes, as each owner last published them. They are
		// never candidates for tracking here. A volume already listed keeps
		// its entry: this device's own observation of a drive outranks a
		// peer's report of it.
		if !matches!(self.filter, VolumeFilter::UntrackedOnly) {
			for volume in crate::service::mounts::peer::published_volumes(&context).await {
				let wanted = matches!(self.filter, VolumeFilter::All) || volume.is_tracked;
				if wanted && !volumes.iter().any(|listed| listed.id == volume.id) {
					volumes.push(volume);
				}
			}
		}

		// Unique bytes come from the source stores' distinct content sizes,
		// rolled up onto the source rows as hashing lands. A whole-volume
		// source is the volume's own figure; without one, subtree sources sum,
		// which can overlap when sources nest and is still measurement rather
		// than guesswork. A volume nothing has indexed stays None, and the
		// client shows nothing rather than an estimate.
		let source_rows = entities::source::Entity::find()
			.filter(entities::source::Column::VolumeUuid.is_not_null())
			.all(db)
			.await?;
		let mut whole_volume: HashMap<Uuid, (i64, i64)> = HashMap::new();
		let mut subtree_sum: HashMap<Uuid, (i64, i64)> = HashMap::new();
		for row in source_rows {
			let (Some(volume_uuid), Some(unique), Some(total)) =
				(row.volume_uuid, row.unique_bytes, row.total_bytes)
			else {
				continue;
			};
			if row.root.as_deref().unwrap_or("").is_empty() {
				whole_volume.insert(volume_uuid, (unique, total));
			} else {
				let entry = subtree_sum.entry(volume_uuid).or_insert((0, 0));
				entry.0 += unique;
				entry.1 += total;
			}
		}
		for volume in &mut volumes {
			let figures = whole_volume
				.get(&volume.id)
				.or_else(|| subtree_sum.get(&volume.id));
			volume.unique_bytes = figures.map(|(unique, _)| (*unique).max(0) as u64);
			volume.indexed_bytes = figures.map(|(_, total)| (*total).max(0) as u64);
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
