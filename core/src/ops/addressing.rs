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
	infra::db::entities::{device, Device},
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
			SdPath::Physical { device_slug, .. } => {
				// For now, physical paths are assumed accessible
				// TODO: Verify device is online using device_slug
				Ok(path.clone())
			}
			// Cloud paths are already resolved (no additional resolution needed)
			SdPath::Cloud { .. } => Ok(path.clone()),
			// If content-based, find the optimal physical path
			SdPath::Content { content_id } => {
				Err(PathResolutionError::NoOnlineInstancesFound(*content_id))
			}
			// Sidecar paths need to be resolved to physical locations
			SdPath::Sidecar {
				content_id,
				kind,
				variant,
				format,
			} => {
				self.resolve_sidecar(context, *content_id, kind, variant, format)
					.await
			}
		}
	}

	/// Resolve multiple SdPaths efficiently in batch
	// pub async fn resolve_batch(
	// 	&self,
	// 	paths: Vec<SdPath>,
	// 	context: &CoreContext,
	// ) -> HashMap<SdPath, Result<SdPath, PathResolutionError>> {
	// 	let mut results = HashMap::new();

	// 	// Partition paths by type
	// 	let mut physical_paths = Vec::new();
	// 	let mut content_paths = Vec::new();

	// 	for path in paths {
	// 		match &path {
	// 			SdPath::Physical { .. } => physical_paths.push(path),
	// 			SdPath::Content { .. } => content_paths.push(path),
	// 		}
	// 	}

	// 	// Pre-compute device status
	// 	let online_devices = self.get_online_devices(context).await;
	// 	let device_metrics = self.get_device_metrics(context).await;

	// 	// Verify physical paths
	// 	for path in physical_paths {
	// 		if let SdPath::Physical { device_id, .. } = &path {
	// 			let result = if online_devices.contains(device_id) {
	// 				Ok(path.clone())
	// 			} else {
	// 				Err(PathResolutionError::DeviceOffline(*device_id))
	// 			};
	// 			results.insert(path.clone(), result);
	// 		}
	// 	}

	// 	// Resolve content paths
	// 	if !content_paths.is_empty() {
	// 		let content_ids: Vec<Uuid> = content_paths
	// 			.iter()
	// 			.filter_map(|p| p.content_id())
	// 			.collect();

	// 		let resolved_content = self
	// 			.resolve_content_paths_batch(context, content_ids, &online_devices, &device_metrics)
	// 			.await;

	// 		for path in content_paths {
	// 			if let SdPath::Content { content_id } = &path {
	// 				let result = resolved_content
	// 					.get(content_id)
	// 					.cloned()
	// 					.unwrap_or_else(|| {
	// 						Err(PathResolutionError::NoOnlineInstancesFound(*content_id))
	// 					});
	// 				results.insert(path.clone(), result);
	// 			}
	// 		}
	// 	}

	// 	results
	// }

	/// Verify that a device is online
	async fn verify_device_online(
		&self,
		context: &CoreContext,
		device_id: Uuid,
	) -> Result<(), PathResolutionError> {
		let current_device_id = crate::device::get_current_device_id();

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
		let mut online = vec![crate::device::get_current_device_id()];

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
		let current_device_id = crate::device::get_current_device_id();
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

	/// Resolve a sidecar path to a physical location
	///
	/// NOTE: This is a placeholder implementation. Full sidecar resolution
	/// will be implemented in VSS-001 task and will include:
	/// - Checking local filesystem
	/// - Querying sidecar_availability for remote devices
	/// - Enqueuing generation if not found
	/// - Returning appropriate ResolvedPath variants
	async fn resolve_sidecar(
		&self,
		_context: &CoreContext,
		content_id: Uuid,
		kind: &crate::domain::sidecar::SidecarKind,
		variant: &crate::domain::sidecar::SidecarVariant,
		_format: &crate::domain::sidecar::SidecarFormat,
	) -> Result<SdPath, PathResolutionError> {
		// TODO: Implement full sidecar resolution
		// For now, return not found to avoid breaking existing code
		Err(PathResolutionError::SidecarNotFound {
			content_id,
			kind: kind.as_str().to_string(),
			variant: variant.as_str().to_string(),
		})
	}

	/// Resolve a content-based path to an optimal physical location
	// async fn resolve_content_path(
	// 	&self,
	// 	context: &CoreContext,
	// 	content_id: Uuid,
	// ) -> Result<SdPath, PathResolutionError> {
	// 	// Get the current library
	// 	let library = context
	// 		.libraries()
	// 		.await
	// 		.get_active_library()
	// 		.await
	// 		.ok_or(PathResolutionError::NoActiveLibrary)?;

	// 	let db = library.db().conn();

	// 	// Find all physical instances of this content
	// 	let instances = self.find_content_instances(db, content_id).await?;

	// 	if instances.is_empty() {
	// 		return Err(PathResolutionError::NoOnlineInstancesFound(content_id));
	// 	}

	// 	// Get device status and metrics
	// 	let online_devices = self.get_online_devices(context).await;
	// 	let device_metrics = self.get_device_metrics(context).await;

	// 	// Build device slug lookup map from DeviceManager
	// 	let device_slug_map: HashMap<Uuid, String> = online_devices
	// 		.iter()
	// 		.filter_map(|&device_id| {
	// 			context.device_manager.get_device_slug(device_id)
	// 				.map(|slug| (device_id, slug))
	// 		})
	// 		.collect();

	// 	// Calculate costs and find the best instance
	// 	self.select_optimal_instance(instances, &online_devices, &device_metrics, &device_slug_map)
	// 		.ok_or(PathResolutionError::NoOnlineInstancesFound(content_id))
	// }

	// /// Resolve multiple content paths efficiently
	// async fn resolve_content_paths_batch(
	// 	&self,
	// 	context: &CoreContext,
	// 	content_ids: Vec<Uuid>,
	// 	online_devices: &[Uuid],
	// 	device_metrics: &HashMap<Uuid, DeviceMetrics>,
	// ) -> HashMap<Uuid, Result<SdPath, PathResolutionError>> {
	// 	let mut results = HashMap::new();

	// 	// Get the current library
	// 	let library = match context.libraries().await.get_active_library().await {
	// 		Some(lib) => lib,
	// 		None => {
	// 			// Return error for all content IDs
	// 			for id in content_ids {
	// 				results.insert(id, Err(PathResolutionError::NoActiveLibrary));
	// 			}
	// 			return results;
	// 		}
	// 	};

	// 	let db = library.db().conn();

	// 	// Build device slug lookup map from DeviceManager
	// 	let device_slug_map: HashMap<Uuid, String> = online_devices
	// 		.iter()
	// 		.filter_map(|&device_id| {
	// 			context.device_manager.get_device_slug(device_id)
	// 				.map(|slug| (device_id, slug))
	// 		})
	// 		.collect();

	// 	// Batch query for all content instances
	// 	match self.find_content_instances_batch(db, &content_ids).await {
	// 		Ok(instances_map) => {
	// 			// Process each content ID
	// 			for content_id in content_ids {
	// 				let result = if let Some(instances) = instances_map.get(&content_id) {
	// 					self.select_optimal_instance(
	// 						instances.clone(),
	// 						online_devices,
	// 						device_metrics,
	// 						&device_slug_map,
	// 					)
	// 					.ok_or(PathResolutionError::NoOnlineInstancesFound(content_id))
	// 				} else {
	// 					Err(PathResolutionError::NoOnlineInstancesFound(content_id))
	// 				};
	// 				results.insert(content_id, result);
	// 			}
	// 		}
	// 		Err(e) => {
	// 			// Return database error for all content IDs
	// 			for id in content_ids {
	// 				results.insert(id, Err(PathResolutionError::DatabaseError(e.to_string())));
	// 			}
	// 		}
	// 	}

	// 	results
	// }

	/// Select the optimal instance based on cost function
	fn select_optimal_instance(
		&self,
		instances: Vec<ContentInstance>,
		online_devices: &[Uuid],
		device_metrics: &HashMap<Uuid, DeviceMetrics>,
		device_slug_map: &HashMap<Uuid, String>,
	) -> Option<SdPath> {
		let current_device_id = crate::device::get_current_device_id();

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
		candidates.first().and_then(|(_, inst)| {
			device_slug_map
				.get(&inst.device_id)
				.map(|device_slug| SdPath::Physical {
					device_slug: device_slug.clone(),
					path: inst.path.clone(),
				})
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
