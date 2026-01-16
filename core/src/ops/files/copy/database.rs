//! Database query support for copy operations
//!
//! Provides instant size and file count estimates by querying
//! Spacedrive's indexed data, enabling immediate progress feedback.

use crate::{
	domain::addressing::SdPath,
	infra::db::entities::{entry, location, Entry},
	ops::indexing::PathResolver,
};
use anyhow::Result;
use sea_orm::{prelude::*, Condition, DatabaseConnection, QuerySelect};
use std::path::{Path, PathBuf};

/// Database query engine for copy preparation
pub struct CopyDatabaseQuery {
	db: DatabaseConnection,
}

impl CopyDatabaseQuery {
	pub fn new(db: DatabaseConnection) -> Self {
		Self { db }
	}

	/// Get instant estimates for multiple source paths
	pub async fn get_estimates_for_paths(&self, sources: &[SdPath]) -> Result<PathEstimates> {
		use crate::ops::indexing::PathResolver;

		let mut total_files = 0u64;
		let mut total_bytes = 0u64;
		let mut indexed_paths = 0u64;

		for source in sources {
			// Use canonical PathResolver for all path types
			match PathResolver::resolve_to_entry(&self.db, source).await {
				Ok(Some(entry)) => {
					let (file_count, total_size) = match entry.kind {
						0 => (1u64, entry.size as u64),                              // File
						1 => (entry.file_count as u64, entry.aggregate_size as u64), // Directory
						_ => (0, 0),
					};
					total_files += file_count;
					total_bytes += total_size;
					indexed_paths += 1;
				}
				Ok(None) | Err(_) => {
					// Entry not found or query failed - will calculate from filesystem later
					continue;
				}
			}
		}

		Ok(PathEstimates {
			file_count: total_files,
			total_size: total_bytes,
			indexed_paths,
			total_paths: sources.len() as u64,
		})
	}

	/// Get estimates for a single path
	async fn get_path_estimates(&self, path: &Path) -> Result<Option<SinglePathEstimate>> {
		let path_str = path.to_string_lossy().to_string();

		// Get all locations to find which one contains this path
		let locations = location::Entity::find().all(&self.db).await?;

		// Check each location to see if it contains this path
		for location in locations {
			// Skip locations without entry_id (not yet synced)
			let Some(entry_id) = location.entry_id else {
				continue;
			};
			// Get the full path of the location's root entry
			let location_path = match PathResolver::get_full_path(&self.db, entry_id).await {
				Ok(path) => path,
				Err(_) => continue, // Skip if we can't get the path
			};

			let location_path_str = location_path.to_string_lossy().to_string();

			// Check if the target path is within this location
			if path_str.starts_with(&location_path_str) {
				// If querying the entire location root, use cached stats
				if path == location_path {
					return Ok(Some(SinglePathEstimate {
						file_count: location.total_file_count as u64,
						total_size: location.total_byte_size as u64,
					}));
				}

				// For paths within the location, we need to find the specific entry
				// by traversing the hierarchy
				let relative_path = match path.strip_prefix(&location_path) {
					Ok(rel) => rel,
					Err(_) => continue,
				};

				// Get path components
				let components: Vec<&str> = relative_path
					.components()
					.filter_map(|c| c.as_os_str().to_str())
					.collect();

				if components.is_empty() {
					continue;
				}

				// Start from the location's root entry and traverse down
				let mut current_parent_id = location.entry_id;
				let mut target_entry = None;

				for component in components {
					if let Some(parent_id) = current_parent_id {
						// Find child with matching name
						let child = entry::Entity::find()
							.filter(entry::Column::ParentId.eq(parent_id))
							.filter(entry::Column::Name.eq(component))
							.one(&self.db)
							.await?;

						match child {
							Some(c) => {
								current_parent_id = Some(c.id);
								target_entry = Some(c);
							}
							None => return Ok(None), // Path not indexed
						}
					} else {
						return Ok(None);
					}
				}

				// Found the target entry
				if let Some(entry) = target_entry {
					let (file_count, total_size) = match entry.kind {
						0 => (1u64, entry.size as u64), // File
						1 => {
							// Directory - use pre-calculated aggregate values
							(entry.file_count as u64, entry.aggregate_size as u64)
						}
						_ => (0, 0), // Symlink or other
					};

					return Ok(Some(SinglePathEstimate {
						file_count,
						total_size,
					}));
				}
			}
		}

		Ok(None) // Path not in any indexed location
	}

	/// Get estimates for a path on a specific device (for cross-device copies)
	/// This queries the synced metadata for files on remote devices
	async fn get_path_estimates_by_device(
		&self,
		device_slug: &str,
		path: &PathBuf,
	) -> Result<Option<SinglePathEstimate>> {
		use crate::infra::db::entities::{device, volume};

		tracing::debug!(
			target: "sd_core::copy_size_query",
			"[SIZE QUERY] Starting query for device '{}' path '{}'",
			device_slug,
			path.display()
		);

		// Find the device by slug
		let device = device::Entity::find()
			.filter(device::Column::Slug.eq(device_slug))
			.one(&self.db)
			.await?;

		let Some(device) = device else {
			tracing::warn!(
				target: "sd_core::copy_size_query",
				"[SIZE QUERY] Device not found: '{}'",
				device_slug
			);
			return Ok(None);
		};

		tracing::debug!(
			target: "sd_core::copy_size_query",
			"[SIZE QUERY] Found device: id={} name={} uuid={}",
			device.id,
			device.name,
			device.uuid
		);

		// Find all volumes for this device (device_id references device.uuid, not device.id)
		let volumes = volume::Entity::find()
			.filter(volume::Column::DeviceId.eq(device.uuid))
			.all(&self.db)
			.await?;

		tracing::debug!(
			target: "sd_core::copy_size_query",
			"[SIZE QUERY] Found {} volumes for device",
			volumes.len()
		);

		let path_str = path.to_string_lossy().to_string();

		// Check each volume's locations
		for volume in volumes {
			let locations = location::Entity::find()
				.filter(location::Column::VolumeId.eq(volume.id))
				.all(&self.db)
				.await?;

			tracing::debug!(
				target: "sd_core::copy_size_query",
				"[SIZE QUERY] Volume {} has {} locations",
				volume.id,
				locations.len()
			);

			for location in locations {
				let Some(entry_id) = location.entry_id else {
					tracing::debug!(
						target: "sd_core::copy_size_query",
						"[SIZE QUERY] Location id={} has no entry_id, skipping",
						location.id
					);
					continue;
				};

				// Get the full path of the location's root entry
				let location_path = match PathResolver::get_full_path(&self.db, entry_id).await {
					Ok(path) => path,
					Err(e) => {
						tracing::warn!(
							target: "sd_core::copy_size_query",
							"[SIZE QUERY] Failed to get path for location id={}: {}",
							location.id,
							e
						);
						continue;
					}
				};

				let location_path_str = location_path.to_string_lossy().to_string();

				tracing::debug!(
					target: "sd_core::copy_size_query",
					"[SIZE QUERY] Checking location id={} with path '{}'",
					location.id,
					location_path_str
				);

				// Check if the target path is within this location
				if path_str.starts_with(&location_path_str) {
					tracing::debug!(
						target: "sd_core::copy_size_query",
						"[SIZE QUERY] Path matches location! Target: '{}', Location: '{}'",
						path_str,
						location_path_str
					);

					// If querying the entire location root, use cached stats
					if path == &location_path {
						tracing::info!(
							target: "sd_core::copy_size_query",
							"[SIZE QUERY] ✓ Using location stats: {} files, {} bytes",
							location.total_file_count,
							location.total_byte_size
						);
						return Ok(Some(SinglePathEstimate {
							file_count: location.total_file_count as u64,
							total_size: location.total_byte_size as u64,
						}));
					}

					// For paths within the location, find the specific entry
					let relative_path = match path.strip_prefix(&location_path) {
						Ok(rel) => rel,
						Err(e) => {
							tracing::warn!(
								target: "sd_core::copy_size_query",
								"[SIZE QUERY] Failed to strip prefix: {}",
								e
							);
							continue;
						}
					};

					let components: Vec<&str> = relative_path
						.components()
						.filter_map(|c| c.as_os_str().to_str())
						.collect();

					if components.is_empty() {
						tracing::debug!(
							target: "sd_core::copy_size_query",
							"[SIZE QUERY] No relative components, skipping"
						);
						continue;
					}

					tracing::debug!(
						target: "sd_core::copy_size_query",
						"[SIZE QUERY] Traversing {} components: {:?}",
						components.len(),
						components
					);

					// Traverse hierarchy to find target entry
					let mut current_parent_id = location.entry_id;
					let mut target_entry = None;

					for component in components {
						if let Some(parent_id) = current_parent_id {
							// Remove extension for entry lookup (extensions stored separately)
							let component_without_ext = if let Some(dot_pos) = component.rfind('.')
							{
								&component[..dot_pos]
							} else {
								component
							};

							// Normalize Unicode spaces (macOS uses special space characters)
							// Replace narrow no-break space (\u{202f}) and other space variants with regular space
							let normalized_name = component_without_ext
								.replace('\u{202f}', " ") // Narrow no-break space
								.replace('\u{00a0}', " ") // Non-breaking space
								.replace('\u{2009}', " "); // Thin space

							let child = entry::Entity::find()
								.filter(entry::Column::ParentId.eq(parent_id))
								.filter(entry::Column::Name.eq(normalized_name))
								.one(&self.db)
								.await?;

							match child {
								Some(c) => {
									tracing::debug!(
										target: "sd_core::copy_size_query",
										"[SIZE QUERY] Found component '{}' (id={})",
										component,
										c.id
									);
									current_parent_id = Some(c.id);
									target_entry = Some(c);
								}
								None => {
									tracing::warn!(
										target: "sd_core::copy_size_query",
										"[SIZE QUERY] Component '{}' not found under parent_id={}",
										component,
										parent_id
									);
									return Ok(None);
								}
							}
						} else {
							return Ok(None);
						}
					}

					// Found the target entry
					if let Some(entry) = target_entry {
						let (file_count, total_size) = match entry.kind {
							0 => (1u64, entry.size as u64),
							1 => (entry.file_count as u64, entry.aggregate_size as u64),
							_ => (0, 0),
						};

						tracing::info!(
							target: "sd_core::copy_size_query",
							"[SIZE QUERY] ✓ Found entry '{}': {} files, {} bytes",
							entry.name,
							file_count,
							total_size
						);

						return Ok(Some(SinglePathEstimate {
							file_count,
							total_size,
						}));
					}
				} else {
					tracing::debug!(
						target: "sd_core::copy_size_query",
						"[SIZE QUERY] Path mismatch - Target: '{}', Location: '{}'",
						path_str,
						location_path_str
					);
				}
			}
		}

		tracing::warn!(
			target: "sd_core::copy_size_query",
			"[SIZE QUERY] ✗ No matching entry found for '{}'",
			path_str
		);
		Ok(None)
	}
}

/// Estimates for a single path
#[derive(Debug, Clone)]
pub struct SinglePathEstimate {
	pub file_count: u64,
	pub total_size: u64,
}

/// Aggregate estimates for multiple paths
#[derive(Debug, Clone)]
pub struct PathEstimates {
	pub file_count: u64,
	pub total_size: u64,
	pub indexed_paths: u64,
	pub total_paths: u64,
}

impl PathEstimates {
	/// Check if we have complete information from the database
	pub fn is_complete(&self) -> bool {
		self.indexed_paths == self.total_paths
	}

	/// Get a confidence score (0.0 to 1.0) for the estimates
	pub fn confidence(&self) -> f32 {
		if self.total_paths == 0 {
			0.0
		} else {
			self.indexed_paths as f32 / self.total_paths as f32
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_path_estimates() {
		let estimates = PathEstimates {
			file_count: 100,
			total_size: 1024 * 1024 * 100, // 100MB
			indexed_paths: 3,
			total_paths: 4,
		};

		assert!(!estimates.is_complete());
		assert_eq!(estimates.confidence(), 0.75);
	}
}
