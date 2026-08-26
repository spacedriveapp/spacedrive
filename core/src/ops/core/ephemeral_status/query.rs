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
	/// Include detailed memory breakdown (more expensive to compute)
	#[serde(default)]
	pub detailed: bool,
}

/// Input for resetting the ephemeral cache
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct EphemeralCacheResetInput {
	/// Confirmation flag to prevent accidental cache clearing
	pub confirm: bool,
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

		// Aggregate stats across every partition (per-source indexes + scratch)
		let mut index_stats = UnifiedIndexStats {
			total_entries: 0,
			path_index_count: 0,
			unique_names: 0,
			interned_strings: 0,
			content_kinds: 0,
			uuid_count: 0,
			memory_bytes: 0,
			total_file_bytes: 0,
			age_seconds: cache.age().as_secs_f64(),
			idle_seconds: f64::MAX,
			memory_breakdown: None,
		};
		let mut breakdown_totals =
			self.input
				.detailed
				.then(|| super::output::MemoryBreakdownStats {
					arena: 0,
					cache: 0,
					registry: 0,
					path_index_overhead: 0,
					path_index_entries: 0,
					entry_uuids_overhead: 0,
					entry_uuids_entries: 0,
					content_kinds_overhead: 0,
					content_kinds_entries: 0,
				});

		for index in cache.all_indexes() {
			let index = index.read().await;
			let stats = index.get_stats();
			index_stats.total_entries += stats.total_entries;
			index_stats.path_index_count += index.path_index_count();
			index_stats.unique_names += stats.unique_names;
			index_stats.interned_strings += stats.interned_strings;
			index_stats.content_kinds += index.content_kinds_count();
			index_stats.uuid_count += stats.uuid_count;
			index_stats.memory_bytes += stats.memory_bytes;
			index_stats.total_file_bytes += stats.total_file_bytes;
			index_stats.idle_seconds = index_stats
				.idle_seconds
				.min(index.idle_time().as_secs_f64());

			if let Some(totals) = breakdown_totals.as_mut() {
				let breakdown = index.detailed_memory_breakdown();
				totals.arena += breakdown.arena;
				totals.cache += breakdown.cache;
				totals.registry += breakdown.registry;
				totals.path_index_overhead += breakdown.path_index_overhead;
				totals.path_index_entries += breakdown.path_index_entries;
				totals.entry_uuids_overhead += breakdown.entry_uuids_overhead;
				totals.entry_uuids_entries += breakdown.entry_uuids_entries;
				totals.content_kinds_overhead += breakdown.content_kinds_overhead;
				totals.content_kinds_entries += breakdown.content_kinds_entries;
			}
		}
		if index_stats.idle_seconds == f64::MAX {
			index_stats.idle_seconds = 0.0;
		}
		index_stats.memory_breakdown = breakdown_totals;

		// Build indexed paths info with child counts, each from its own partition
		let mut indexed_paths = Vec::new();
		for path in all_indexed_paths {
			// Apply path filter if provided
			if let Some(ref filter) = self.input.path_filter {
				if !path.to_string_lossy().contains(filter) {
					continue;
				}
			}

			let child_count = {
				let index = cache.resolve_index(&path);
				let index = index.read().await;
				index.list_directory(&path).map(|c| c.len()).unwrap_or(0)
			};

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

		let sources = cache
			.sources()
			.into_iter()
			.map(|s| super::output::EphemeralSourceInfo {
				id: s.id,
				root: s.root,
				volume_uuid: s.volume_uuid,
				attached: s.attached,
				restored: s.restored,
				last_seen_secs: s.last_seen_secs,
				entry_count: s.entry_count,
				total_bytes: s.total_bytes,
				directory: s.directory,
				thumbs_path: s.thumbs_path,
			})
			.collect();

		Ok(EphemeralCacheStatus {
			indexed_paths_count: cache_stats.indexed_paths,
			indexing_in_progress_count: cache_stats.indexing_in_progress,
			index_stats,
			indexed_paths,
			paths_in_progress: filtered_in_progress,
			sources,
			// Legacy fields
			total_indexes: None,
			indexing_in_progress: None,
			indexes: Vec::new(),
		})
	}
}

crate::register_core_query!(EphemeralCacheStatusQuery, "core.ephemeral_status");
