//! Path resolution operations for the Virtual Distributed File System
//!
//! This module contains the active logic (verbs) that operates on SdPath
//! data structures to resolve content-based paths to optimal physical locations.

use sea_orm::{prelude::*, QuerySelect};
use std::collections::HashMap;
use uuid::Uuid;

use crate::{
	context::CoreContext,
	domain::addressing::{PathResolutionError, SdPath},
	infrastructure::database::entities::{
		content_identity, device, entry, location, ContentIdentity, Device, Entry, Location,
	},
};

/// The PathResolver service handles resolution of SdPath instances
/// to optimal physical locations based on device availability and
/// performance characteristics.
pub struct PathResolver;

impl PathResolver {
	/// Resolve a single SdPath to an optimal physical location
	pub async fn resolve(
		&self,
		path: &SdPath,
		context: &CoreContext,
	) -> Result<SdPath, PathResolutionError> {
		match path {
			// If already physical, just verify the device is online
			SdPath::Physical { device_id, .. } => {
				self.verify_device_online(context, *device_id).await?;
				Ok(path.clone())
			}
			// If content-based, find the optimal physical path
			SdPath::Content { content_id } => self.resolve_content_path(context, *content_id).await,
		}
	}

	/// Resolve multiple SdPaths efficiently in batch
	pub async fn resolve_batch(
		&self,
		paths: Vec<SdPath>,
		context: &CoreContext,
	) -> HashMap<SdPath, Result<SdPath, PathResolutionError>> {
		let mut results = HashMap::new();

		// Partition paths by type
		let mut physical_paths = Vec::new();
		let mut content_paths = Vec::new();

		for path in paths {
			match &path {
				SdPath::Physical { .. } => physical_paths.push(path),
				SdPath::Content { .. } => content_paths.push(path),
			}
		}

		// Pre-compute device status
		let online_devices = self.get_online_devices(context).await;
		let device_metrics = self.get_device_metrics(context).await;

		// Verify physical paths
		for path in physical_paths {
			if let SdPath::Physical { device_id, .. } = &path {
				let result = if online_devices.contains(device_id) {
					Ok(path.clone())
				} else {
					Err(PathResolutionError::DeviceOffline(*device_id))
				};
				results.insert(path.clone(), result);
			}
		}

		// Resolve content paths
		if !content_paths.is_empty() {
			let content_ids: Vec<Uuid> = content_paths
				.iter()
				.filter_map(|p| p.content_id())
				.collect();

			let resolved_content = self
				.resolve_content_paths_batch(context, content_ids, &online_devices, &device_metrics)
				.await;

			for path in content_paths {
				if let SdPath::Content { content_id } = &path {
					let result = resolved_content
						.get(content_id)
						.cloned()
						.unwrap_or_else(|| {
							Err(PathResolutionError::NoOnlineInstancesFound(*content_id))
						});
					results.insert(path.clone(), result);
				}
			}
		}

		results
	}

	/// Verify that a device is online
	async fn verify_device_online(
		&self,
		context: &CoreContext,
		device_id: Uuid,
	) -> Result<(), PathResolutionError> {
		let current_device_id = crate::shared::utils::get_current_device_id();

		// Local device is always "online"
		if device_id == current_device_id {
			return Ok(());
		}

		// Check with networking service
		let is_online = if let Some(networking) = context.get_networking().await {
			// Check if device is in connected devices list
			networking
				.get_connected_devices()
				.await
				.iter()
				.any(|dev| dev.device_id == device_id)
		} else {
			false
		};

		if is_online {
			Ok(())
		} else {
			Err(PathResolutionError::DeviceOffline(device_id))
		}
	}

	/// Get list of currently online devices
	async fn get_online_devices(&self, context: &CoreContext) -> Vec<Uuid> {
		let mut online = vec![crate::shared::utils::get_current_device_id()];

		if let Some(networking) = context.get_networking().await {
			for device in networking.get_connected_devices().await {
				online.push(device.device_id);
			}
		}

		online
	}

	/// Get device performance metrics for cost calculation
	async fn get_device_metrics(&self, context: &CoreContext) -> HashMap<Uuid, DeviceMetrics> {
		let mut metrics = HashMap::new();

		// Local device has zero latency
		let current_device_id = crate::shared::utils::get_current_device_id();
		metrics.insert(
			current_device_id,
			DeviceMetrics {
				latency_ms: 0,
				bandwidth_mbps: 1000, // Assume high local bandwidth
			},
		);

		// Get metrics from networking service
		if let Some(networking) = context.get_networking().await {
			for device in networking.get_connected_devices().await {
				// TODO: Get actual metrics from networking service
				metrics.insert(
					device.device_id,
					DeviceMetrics {
						latency_ms: 50,      // Placeholder
						bandwidth_mbps: 100, // Placeholder
					},
				);
			}
		}

		metrics
	}

	/// Resolve a content-based path to an optimal physical location
	async fn resolve_content_path(
		&self,
		context: &CoreContext,
		content_id: Uuid,
	) -> Result<SdPath, PathResolutionError> {
		// Get the current library
		let library = context
			.library_manager
			.get_primary_library()
			.await
			.ok_or(PathResolutionError::NoActiveLibrary)?;

		let db = library.db().conn();

		// Find all physical instances of this content
		let instances = self.find_content_instances(db, content_id).await?;

		if instances.is_empty() {
			return Err(PathResolutionError::NoOnlineInstancesFound(content_id));
		}

		// Get device status and metrics
		let online_devices = self.get_online_devices(context).await;
		let device_metrics = self.get_device_metrics(context).await;

		// Calculate costs and find the best instance
		self.select_optimal_instance(instances, &online_devices, &device_metrics)
			.ok_or(PathResolutionError::NoOnlineInstancesFound(content_id))
	}

	/// Resolve multiple content paths efficiently
	async fn resolve_content_paths_batch(
		&self,
		context: &CoreContext,
		content_ids: Vec<Uuid>,
		online_devices: &[Uuid],
		device_metrics: &HashMap<Uuid, DeviceMetrics>,
	) -> HashMap<Uuid, Result<SdPath, PathResolutionError>> {
		let mut results = HashMap::new();

		// Get the current library
		let library = match context.library_manager.get_primary_library().await {
			Some(lib) => lib,
			None => {
				// Return error for all content IDs
				for id in content_ids {
					results.insert(id, Err(PathResolutionError::NoActiveLibrary));
				}
				return results;
			}
		};

		let db = library.db().conn();

		// Batch query for all content instances
		match self.find_content_instances_batch(db, &content_ids).await {
			Ok(instances_map) => {
				// Process each content ID
				for content_id in content_ids {
					let result = if let Some(instances) = instances_map.get(&content_id) {
						self.select_optimal_instance(
							instances.clone(),
							online_devices,
							device_metrics,
						)
						.ok_or(PathResolutionError::NoOnlineInstancesFound(content_id))
					} else {
						Err(PathResolutionError::NoOnlineInstancesFound(content_id))
					};
					results.insert(content_id, result);
				}
			}
			Err(e) => {
				// Return database error for all content IDs
				for id in content_ids {
					results.insert(id, Err(PathResolutionError::DatabaseError(e.to_string())));
				}
			}
		}

		results
	}

	/// Find all physical instances of a content ID
	async fn find_content_instances<C: ConnectionTrait>(
		&self,
		db: &C,
		content_id: Uuid,
	) -> Result<Vec<ContentInstance>, DbErr> {
		// First find the ContentIdentity by UUID
		let content_identity = ContentIdentity::find()
			.filter(content_identity::Column::Uuid.eq(Some(content_id)))
			.one(db)
			.await?
			.ok_or_else(|| {
				DbErr::RecordNotFound(format!("Content identity not found: {}", content_id))
			})?;

		// Query to find all entries with this content_identity.id
		let entries = Entry::find()
			.filter(entry::Column::ContentId.eq(Some(content_identity.id)))
			.all(db)
			.await?;

		// For each entry, get its location
		let mut entries_with_location = Vec::new();
		for entry in entries {
			if let Some(location) = Location::find()
				.filter(location::Column::EntryId.eq(entry.id))
				.one(db)
				.await?
			{
				entries_with_location.push((entry, location));
			}
		}

		let mut instances = Vec::new();
		for (entry, location) in entries_with_location {
			// Get device info from location
			if let Some(device) = Device::find()
				.filter(device::Column::Id.eq(location.device_id))
				.one(db)
				.await?
			{
				// Build the full path using PathResolver
				let path = crate::operations::indexing::path_resolver::PathResolver::get_full_path(
					db, entry.id,
				)
				.await?;

				instances.push(ContentInstance {
					device_id: device.uuid,
					path,
				});
			}
		}

		Ok(instances)
	}

	/// Find physical instances for multiple content IDs in batch
	async fn find_content_instances_batch<C: ConnectionTrait>(
		&self,
		db: &C,
		content_ids: &[Uuid],
	) -> Result<HashMap<Uuid, Vec<ContentInstance>>, DbErr> {
		// First find all ContentIdentities by UUIDs
		let mut content_identities: Vec<content_identity::Model> = Vec::new();
		let chunk_size: usize = 900;
		for chunk in content_ids.chunks(chunk_size) {
			let mut batch = ContentIdentity::find()
				.filter(content_identity::Column::Uuid.is_in(chunk.iter().copied().map(Some)))
				.all(db)
				.await?;
			content_identities.append(&mut batch);
		}

		// Create a map from UUID to database ID
		let uuid_to_id: HashMap<Uuid, i32> = content_identities
			.iter()
			.filter_map(|ci| ci.uuid.map(|uuid| (uuid, ci.id)))
			.collect();

		// Get database IDs for the query
		let db_ids: Vec<i32> = content_identities.iter().map(|ci| ci.id).collect();

		// Query to find all entries with these content_identity IDs
		let mut entries: Vec<entry::Model> = Vec::new();
		for chunk in db_ids.chunks(chunk_size) {
			let mut batch = Entry::find()
				.filter(entry::Column::ContentId.is_in(chunk.iter().copied().map(Some)))
				.all(db)
				.await?;
			entries.append(&mut batch);
		}

		// Group by content_id
		let mut instances_map: HashMap<Uuid, Vec<ContentInstance>> = HashMap::new();

		// Get locations for all entries in batch
		let entry_ids: Vec<i32> = entries.iter().map(|e| e.id).collect();
		let mut locations: Vec<location::Model> = Vec::new();
		for chunk in entry_ids.chunks(chunk_size) {
			let mut batch = Location::find()
				.filter(location::Column::EntryId.is_in(chunk.to_vec()))
				.all(db)
				.await?;
			locations.append(&mut batch);
		}

		// Create a map for quick lookup
		let location_map: HashMap<i32, location::Model> = locations
			.into_iter()
			.map(|loc| (loc.entry_id, loc))
			.collect();

		// Get devices for all locations
		let device_ids: Vec<i32> = location_map.values().map(|loc| loc.device_id).collect();
		let mut devices: Vec<device::Model> = Vec::new();
		for chunk in device_ids.chunks(chunk_size) {
			let mut batch = Device::find()
				.filter(device::Column::Id.is_in(chunk.to_vec()))
				.all(db)
				.await?;
			devices.append(&mut batch);
		}

		let device_map: HashMap<i32, device::Model> =
			devices.into_iter().map(|dev| (dev.id, dev)).collect();

		// Create reverse map from database ID to UUID
		let id_to_uuid: HashMap<i32, Uuid> =
			uuid_to_id.iter().map(|(uuid, id)| (*id, *uuid)).collect();

		for entry in entries {
			if let Some(content_db_id) = entry.content_id {
				if let Some(content_uuid) = id_to_uuid.get(&content_db_id) {
					if let Some(location) = location_map.get(&entry.id) {
						if let Some(device) = device_map.get(&location.device_id) {
							// Build the full path
							let path = crate::operations::indexing::path_resolver::PathResolver::get_full_path(
                                db,
                                entry.id,
                            )
                            .await?;

							let instance = ContentInstance {
								device_id: device.uuid,
								path,
							};

							instances_map
								.entry(*content_uuid)
								.or_insert_with(Vec::new)
								.push(instance);
						}
					}
				}
			}
		}

		Ok(instances_map)
	}

	/// Select the optimal instance based on cost function
	fn select_optimal_instance(
		&self,
		instances: Vec<ContentInstance>,
		online_devices: &[Uuid],
		device_metrics: &HashMap<Uuid, DeviceMetrics>,
	) -> Option<SdPath> {
		let current_device_id = crate::shared::utils::get_current_device_id();

		let mut candidates: Vec<(f64, &ContentInstance)> = instances
			.iter()
			.filter(|inst| online_devices.contains(&inst.device_id))
			.map(|inst| {
				let cost = self.calculate_instance_cost(inst, current_device_id, device_metrics);
				(cost, inst)
			})
			.collect();

		// Sort by cost (lower is better)
		candidates.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

		// Return the best instance
		candidates.first().map(|(_, inst)| SdPath::Physical {
			device_id: inst.device_id,
			path: inst.path.clone(),
		})
	}

	/// Calculate the cost of accessing an instance
	fn calculate_instance_cost(
		&self,
		instance: &ContentInstance,
		current_device_id: Uuid,
		device_metrics: &HashMap<Uuid, DeviceMetrics>,
	) -> f64 {
		// Priority 1: Local device (cost = 0)
		if instance.device_id == current_device_id {
			return 0.0;
		}

		// Priority 2: Network latency
		let metrics = device_metrics.get(&instance.device_id);
		let latency_cost = metrics.map(|m| m.latency_ms as f64).unwrap_or(1000.0);

		// Priority 3: Bandwidth (inverse relationship - higher bandwidth = lower cost)
		let bandwidth_cost = metrics
			.map(|m| 1000.0 / m.bandwidth_mbps as f64)
			.unwrap_or(10.0);

		// Combined cost (latency weighted more heavily)
		latency_cost * 0.8 + bandwidth_cost * 0.2
	}
}

/// Information about a physical instance of content
#[derive(Clone)]
struct ContentInstance {
	device_id: Uuid,
	path: std::path::PathBuf,
}

/// Device performance metrics for cost calculation
struct DeviceMetrics {
	latency_ms: u32,
	bandwidth_mbps: u32,
}

/// Integrate resolve method directly into SdPath
impl SdPath {
	/// Resolve this path using the provided PathResolver
	pub async fn resolve_with(
		&self,
		resolver: &PathResolver,
		context: &CoreContext,
	) -> Result<SdPath, PathResolutionError> {
		resolver.resolve(self, context).await
	}
}
