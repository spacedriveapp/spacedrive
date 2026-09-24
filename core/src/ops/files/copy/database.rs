//! Instant size and file-count estimates for a copy.
//!
//! The arena maintains `subtree_bytes` and `file_count` along the ancestor
//! chain as it is written, so the numbers a copy needs for its progress bar are
//! already computed. Reading them costs a lookup rather than the walk the
//! filesystem would charge for the same answer.

use crate::{domain::addressing::SdPath, ops::indexing::VolumeIndex};
use anyhow::Result;

/// Estimate engine over the volume index.
pub struct CopyDatabaseQuery<'a> {
	index: &'a VolumeIndex,
}

impl<'a> CopyDatabaseQuery<'a> {
	pub fn new(index: &'a VolumeIndex) -> Self {
		Self { index }
	}

	/// Estimates for several source paths at once.
	///
	/// A path the arena does not cover contributes nothing and is counted as
	/// unindexed, which is what `confidence` reports: the caller falls back to
	/// walking the filesystem when the answer is partial.
	pub async fn get_estimates_for_paths(&self, sources: &[SdPath]) -> Result<PathEstimates> {
		let cache = self.index;
		let mut estimates = PathEstimates {
			file_count: 0,
			total_size: 0,
			indexed_paths: 0,
			total_paths: sources.len() as u64,
		};

		for source in sources {
			let SdPath::Physical { path, .. } = source else {
				continue;
			};

			cache.ensure_restored(path).await;
			let Some(index) = cache.get_for_search(path) else {
				continue;
			};

			let index = index.read().await;
			let Some(metadata) = index.get_entry_ref(path) else {
				continue;
			};

			// A file is one file of its own size; a directory answers with the
			// rollup the arena keeps for it.
			if metadata.kind == crate::ops::indexing::state::EntryKind::Directory {
				let Some(bytes) = index.subtree_size(path) else {
					continue;
				};
				estimates.total_size += bytes;
				estimates.file_count += index.subtree_file_count(path).unwrap_or(0) as u64;
			} else {
				estimates.total_size += metadata.size;
				estimates.file_count += 1;
			}

			estimates.indexed_paths += 1;
		}

		Ok(estimates)
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
