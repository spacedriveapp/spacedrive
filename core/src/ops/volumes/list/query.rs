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
use sea_orm::EntityTrait;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{collections::HashMap, sync::Arc};

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

		// Get tracked volumes from database
		let tracked_volumes = entities::volume::Entity::find().all(db).await?;

		// Create a map of tracked volumes by fingerprint
		let mut tracked_map: HashMap<String, entities::volume::Model> = tracked_volumes
			.into_iter()
			.map(|v| (v.fingerprint.clone(), v))
			.collect();

		let volume_manager = &context.volume_manager;
		let mut volume_items = Vec::new();

		match self.filter {
			VolumeFilter::TrackedOnly => {
				// Only return tracked volumes
				for tracked_vol in tracked_map.values() {
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
					});
				}
			}
			VolumeFilter::UntrackedOnly => {
				// Get all detected volumes from volume manager
				let all_volumes = volume_manager.get_all_volumes().await;

				// Only return volumes that are NOT tracked
				for vol in all_volumes {
					if !tracked_map.contains_key(&vol.fingerprint.0) {
						volume_items.push(super::output::VolumeItem {
							id: vol.id,
							name: vol.name.clone(),
							fingerprint: vol.fingerprint.clone(),
							volume_type: format!("{:?}", vol.volume_type),
							mount_point: Some(vol.mount_point.to_string_lossy().to_string()),
							is_tracked: false,
							is_online: vol.is_mounted,
						});
					}
				}
			}
			VolumeFilter::All => {
				// Get all detected volumes
				let all_volumes = volume_manager.get_all_volumes().await;

				// Add all volumes, marking which are tracked
				for vol in all_volumes {
					let is_tracked = tracked_map.contains_key(&vol.fingerprint.0);
					let (id, name, mount_point) = if is_tracked {
						let tracked = tracked_map.remove(&vol.fingerprint.0).unwrap();
						(
							tracked.uuid,
							tracked.display_name.unwrap_or_else(|| vol.name.clone()),
							tracked
								.mount_point
								.or_else(|| Some(vol.mount_point.to_string_lossy().to_string())),
						)
					} else {
						(
							vol.id,
							vol.name.clone(),
							Some(vol.mount_point.to_string_lossy().to_string()),
						)
					};

					volume_items.push(super::output::VolumeItem {
						id,
						name,
						fingerprint: vol.fingerprint.clone(),
						volume_type: format!("{:?}", vol.volume_type),
						mount_point,
						is_tracked,
						is_online: vol.is_mounted,
					});
				}
			}
		}

		Ok(VolumeListOutput {
			volumes: volume_items,
		})
	}
}

crate::register_library_query!(VolumeListQuery, "volumes.list");
