//! Ephemeral index cache status query
//!
//! Provides a snapshot of the unified ephemeral index for debugging.

use super::output::*;
use crate::{
	context::CoreContext,
	infra::query::{CoreQuery, QueryResult},
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

/// Input for the ephemeral cache status query
#[derive(Debug, Clone, Serialize, Deserialize, Type, Default)]
pub struct EphemeralCacheStatusInput {
	/// Optional: only include indexed paths containing this substring
	#[serde(default)]
	pub path_filter: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct EphemeralCacheStatusQuery {
	input: EphemeralCacheStatusInput,
}

impl CoreQuery for EphemeralCacheStatusQuery {
	type Input = EphemeralCacheStatusInput;
	type Output = EphemeralCacheStatus;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		_session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let cache = context.ephemeral_cache();

		// Get cache stats
		let cache_stats = cache.stats();
		let all_indexed_paths = cache.indexed_paths();
		let paths_in_progress = cache.paths_in_progress();

		// Get the global index for detailed stats
		let global_index = cache.get_global_index();
		let index = global_index.read().await;
		let stats = index.get_stats();

		// Build unified index stats
		let index_stats = UnifiedIndexStats {
			total_entries: stats.total_entries,
			path_index_count: index.path_index_count(),
			unique_names: stats.unique_names,
			interned_strings: stats.interned_strings,
			content_kinds: index.content_kinds_count(),
			memory_bytes: stats.memory_bytes,
			age_seconds: cache.age().as_secs_f64(),
			idle_seconds: index.idle_time().as_secs_f64(),
		};

		// Build indexed paths info with child counts
		let mut indexed_paths = Vec::new();
		for path in all_indexed_paths {
			// Apply path filter if provided
			if let Some(ref filter) = self.input.path_filter {
				if !path.to_string_lossy().contains(filter) {
					continue;
				}
			}

			// Get child count for this directory
			let child_count = index.list_directory(&path).map(|c| c.len()).unwrap_or(0);

			indexed_paths.push(IndexedPathInfo { path, child_count });
		}

		// Sort by path for consistent output
		indexed_paths.sort_by(|a, b| a.path.cmp(&b.path));

		// Filter paths in progress
		let filtered_in_progress: Vec<_> = if let Some(ref filter) = self.input.path_filter {
			paths_in_progress
				.into_iter()
				.filter(|p| p.to_string_lossy().contains(filter))
				.collect()
		} else {
			paths_in_progress
		};

		Ok(EphemeralCacheStatus {
			indexed_paths_count: cache_stats.indexed_paths,
			indexing_in_progress_count: cache_stats.indexing_in_progress,
			index_stats,
			indexed_paths,
			paths_in_progress: filtered_in_progress,
			// Legacy fields
			total_indexes: None,
			indexing_in_progress: None,
			indexes: Vec::new(),
		})
	}
}

crate::register_core_query!(EphemeralCacheStatusQuery, "core.ephemeral_status");
