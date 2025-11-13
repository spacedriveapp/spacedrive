use super::{MissingSidecar, SidecarSource, SidecarSyncFilters, SidecarTransferPlan};
use crate::{
	device::get_current_device_id,
	infra::db::entities::{sidecar, sidecar_availability},
	library::Library,
	ops::sidecar::{SidecarKind, SidecarVariant},
	service::{network::NetworkingService, sidecar_manager::SidecarManager},
};
use anyhow::Result;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};
use std::{collections::HashMap, sync::Arc};
use tracing::debug;
use uuid::Uuid;

pub struct SidecarSyncCoordinator {
	library: Arc<Library>,
	networking: Arc<NetworkingService>,
	sidecar_manager: Arc<SidecarManager>,
}

impl SidecarSyncCoordinator {
	pub fn new(
		library: Arc<Library>,
		networking: Arc<NetworkingService>,
		sidecar_manager: Arc<SidecarManager>,
	) -> Self {
		Self {
			library,
			networking,
			sidecar_manager,
		}
	}

	/// Discover sidecars that exist in DB but aren't available locally
	pub async fn discover_missing_sidecars(
		&self,
		filters: SidecarSyncFilters,
	) -> Result<Vec<MissingSidecar>> {
		let db = self.library.db();
		let device_uuid = get_current_device_id();

		// Build base query for sidecars
		let mut query = sidecar::Entity::find();

		// Apply kind filter
		if let Some(kinds) = &filters.kinds {
			let kind_strs: Vec<String> = kinds.iter().map(|k| k.to_string()).collect();
			query = query.filter(sidecar::Column::Kind.is_in(kind_strs));
		}

		// Apply content UUID filter
		if let Some(content_uuids) = &filters.content_uuids {
			query = query.filter(sidecar::Column::ContentUuid.is_in(content_uuids.clone()));
		}

		// Only ready sidecars
		query = query.filter(sidecar::Column::Status.eq("ready"));

		// Apply pagination
		if let Some(max_count) = filters.max_count {
			query = query.limit(max_count as u64);
		}

		// Execute query
		let all_sidecars = query.all(db.conn()).await?;

		debug!(
			"Found {} total sidecars matching filters",
			all_sidecars.len()
		);

		// Now filter to only those we don't have locally
		let mut missing = Vec::new();

		for sc in all_sidecars {
			// Check if we have availability record
			let availability = sidecar_availability::Entity::find()
				.filter(sidecar_availability::Column::ContentUuid.eq(sc.content_uuid))
				.filter(sidecar_availability::Column::Kind.eq(&sc.kind))
				.filter(sidecar_availability::Column::Variant.eq(&sc.variant))
				.filter(sidecar_availability::Column::DeviceUuid.eq(device_uuid))
				.one(db.conn())
				.await?;

			// If no record or has = false, it's missing
			if availability.is_none() || !availability.unwrap().has {
				missing.push(MissingSidecar {
					sidecar_uuid: sc.uuid,
					content_uuid: sc.content_uuid,
					kind: SidecarKind::from_str(&sc.kind).map_err(anyhow::Error::msg)?,
					variant: SidecarVariant::new(&sc.variant),
					format: sc.format.as_str().try_into().map_err(anyhow::Error::msg)?,
					size: sc.size,
					checksum: sc.checksum,
				});
			}
		}

		debug!("Discovered {} missing sidecars", missing.len());

		Ok(missing)
	}

	/// Query remote devices for sidecar availability
	pub async fn query_remote_availability(
		&self,
		missing: &[MissingSidecar],
	) -> Result<HashMap<Uuid, Vec<SidecarSource>>> {
		if missing.is_empty() {
			return Ok(HashMap::new());
		}

		let db = self.library.db();
		let current_device = get_current_device_id();

		// Build list of content UUIDs to query
		let content_uuids: Vec<Uuid> = missing.iter().map(|m| m.content_uuid).collect();

		// Query availability across all devices (except current)
		let availability = sidecar_availability::Entity::find()
			.filter(sidecar_availability::Column::ContentUuid.is_in(content_uuids))
			.filter(sidecar_availability::Column::Has.eq(true))
			.filter(sidecar_availability::Column::DeviceUuid.ne(current_device))
			.all(db.conn())
			.await?;

		debug!(
			"Found {} availability records from remote devices",
			availability.len()
		);

		// Get online devices
		let online_devices = self.networking.get_connected_devices().await;
		let online_device_ids: std::collections::HashSet<Uuid> = online_devices
			.iter()
			.map(|d| d.device_id)
			.collect();

		// Build map of content_uuid -> sources, filtering to online devices
		let mut sources_map: HashMap<Uuid, Vec<SidecarSource>> = HashMap::new();

		for avail in availability {
			// Check if device is online
			if !online_device_ids.contains(&avail.device_uuid) {
				continue;
			}

			// Match with missing sidecar (ensure kind/variant match)
			let matches = missing.iter().any(|m| {
				m.content_uuid == avail.content_uuid
					&& m.kind.as_str() == avail.kind
					&& m.variant.as_str() == avail.variant
			});

			if matches {
				sources_map
					.entry(avail.content_uuid)
					.or_default()
					.push(SidecarSource {
						device_uuid: avail.device_uuid,
						last_seen_at: avail.last_seen_at,
						verified_checksum: avail.checksum,
					});
			}
		}

		debug!(
			"Found online sources for {} sidecars",
			sources_map.len()
		);

		Ok(sources_map)
	}

	/// Plan transfers by selecting best source for each sidecar
	pub fn plan_transfers(
		&self,
		missing: Vec<MissingSidecar>,
		sources: HashMap<Uuid, Vec<SidecarSource>>,
	) -> Vec<SidecarTransferPlan> {
		missing
			.into_iter()
			.filter_map(|sidecar| {
				let available_sources = sources.get(&sidecar.content_uuid)?;

				if available_sources.is_empty() {
					return None;
				}

				// Select best source (for now, just pick first one - latency tracking TODO)
				let best_source = available_sources.first()?;

				Some(SidecarTransferPlan {
					sidecar,
					source_device: best_source.device_uuid,
				})
			})
			.collect()
	}
}
