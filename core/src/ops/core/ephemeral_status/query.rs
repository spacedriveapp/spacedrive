//! Ephemeral index cache status query
//!
//! Provides a snapshot of all cached ephemeral indexes for debugging.

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
	/// Optional: only include indexes for paths containing this substring
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

		// Get basic cache stats
		let cache_stats = cache.stats();
		let cached_paths = cache.cached_paths();

		// Gather detailed info for each index
		let mut indexes = Vec::new();

		for path in cached_paths {
			// Apply path filter if provided
			if let Some(ref filter) = self.input.path_filter {
				if !path.to_string_lossy().contains(filter) {
					continue;
				}
			}

			// Check if indexing is in progress
			let indexing_in_progress = cache.is_indexing(&path);

			// Try to get the index to read its internal stats
			if let Some(index_arc) = cache.get(&path) {
				let index = index_arc.read().await;
				let stats = index.get_stats();

				let info = EphemeralIndexInfo {
					root_path: index.root_path.clone(),
					indexing_in_progress,
					total_entries: stats.total_entries,
					path_index_count: index.path_index_count(),
					unique_names: stats.unique_names,
					interned_strings: stats.interned_strings,
					content_kinds: index.content_kinds_count(),
					memory_bytes: stats.memory_bytes,
					age_seconds: index.age().as_secs_f64(),
					idle_seconds: index.idle_time().as_secs_f64(),
					job_stats: JobStats {
						files: index.stats.files,
						dirs: index.stats.dirs,
						symlinks: index.stats.symlinks,
						bytes: index.stats.bytes,
					},
				};

				indexes.push(info);
			}
		}

		// Sort by root path for consistent output
		indexes.sort_by(|a, b| a.root_path.cmp(&b.root_path));

		Ok(EphemeralCacheStatus {
			total_indexes: cache_stats.total_entries,
			indexing_in_progress: cache_stats.indexing_count,
			indexes,
		})
	}
}

crate::register_core_query!(EphemeralCacheStatusQuery, "core.ephemeral_status");
