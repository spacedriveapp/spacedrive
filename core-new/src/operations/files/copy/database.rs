//! Database query support for copy operations
//!
//! Provides instant size and file count estimates by querying
//! Spacedrive's indexed data, enabling immediate progress feedback.

use crate::{
	infrastructure::database::entities::{entry, location},
	shared::types::SdPath,
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
		let mut total_files = 0u64;
		let mut total_bytes = 0u64;
		let mut indexed_paths = 0u64;

		for source in sources {
			if let Some(local_path) = source.as_local_path() {
				if let Some(estimates) = self.get_path_estimates(local_path).await? {
					total_files += estimates.file_count;
					total_bytes += estimates.total_size;
					indexed_paths += 1;
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

		// Find the location that contains this path
		// For now, we'll do a simple prefix match
		let locations_list = location::Entity::find().all(&self.db).await?;

		let location = locations_list
			.into_iter()
			.filter(|loc| path_str.starts_with(&loc.path))
			.max_by_key(|loc| loc.path.len()); // Get the most specific match

		let location = match location {
			Some(loc) => loc,
			None => return Ok(None), // Path not in any indexed location
		};

		let location_path = PathBuf::from(&location.path);

		// Calculate the relative path within the location
		let relative_path = path
			.strip_prefix(&location_path)
			.map(|p| p.to_string_lossy().to_string())
			.unwrap_or_default();

		// If we're querying the entire location, use cached stats
		if relative_path.is_empty() {
			return Ok(Some(SinglePathEstimate {
				file_count: location.total_file_count as u64,
				total_size: location.total_byte_size as u64,
			}));
		}

		// For specific paths within locations, we need to query entries
		// This is a simplified version - in reality we'd need more complex queries
		// to handle directory aggregation properly
		println!("relative_path: {}", relative_path);

		// Split the relative path to get parent directory and name
		let (parent_path, name) = if let Some(pos) = relative_path.rfind('/') {
			(
				relative_path[..pos].to_string(),
				relative_path[pos + 1..].to_string(),
			)
		} else {
			(String::new(), relative_path)
		};

		// Query for the specific entry
		let entry = entry::Entity::find()
			.filter(
				Condition::all()
					.add(entry::Column::LocationId.eq(location.id))
					.add(entry::Column::RelativePath.eq(parent_path))
					.add(entry::Column::Name.eq(name)),
			)
			.one(&self.db)
			.await?;

		match entry {
			Some(entry) => {
				let (file_count, total_size) = match entry.kind {
					0 => (1u64, entry.size as u64), // File
					1 => {
						// Directory
						// Use pre-calculated aggregate values
						let files = entry.file_count as u64;
						let size = entry.aggregate_size as u64;
						(files, size)
					}
					_ => (0, 0), // Symlink or other
				};

				Ok(Some(SinglePathEstimate {
					file_count,
					total_size,
				}))
			}
			None => {
				// Path exists in location but not yet indexed
				Ok(None)
			}
		}
	}
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

/// Estimates for a single path
#[derive(Debug, Clone)]
pub struct SinglePathEstimate {
	pub file_count: u64,
	pub total_size: u64,
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
