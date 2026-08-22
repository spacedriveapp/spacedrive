//! Query router: fan-out search across sources.
//!
//! Every source is searched through its record table, so results from different data
//! types come back in one shape. Durable overlays are composed onto the hits
//! before they are returned — callers never join the layers themselves.

use std::collections::HashMap;
use std::sync::Arc;

use crate::db::TemporalFilter;
use crate::error::Result;
use crate::registry::Registry;
use crate::search::{SearchFilter, SearchResult};
use crate::source::SourceManager;

const DEFAULT_LIMIT: usize = 20;

/// Routes search queries across all sources.
pub struct SearchRouter {
	pub(crate) registry: Arc<Registry>,
	pub(crate) sources: Arc<SourceManager>,
}

impl SearchRouter {
	pub fn new(registry: Arc<Registry>, sources: Arc<SourceManager>) -> Self {
		Self { registry, sources }
	}

	/// Search across all (or filtered) sources.
	pub async fn search(
		&self,
		query: &str,
		filter: Option<SearchFilter>,
	) -> Result<Vec<SearchResult>> {
		let filter = filter.unwrap_or_default();
		let limit = filter.limit.unwrap_or(DEFAULT_LIMIT);

		let all_sources = self.registry.list_sources().await?;
		let sources_to_search: Vec<_> = all_sources
			.into_iter()
			.filter(|s| {
				if let Some(ref source_id) = filter.source_id {
					return &s.id == source_id;
				}
				if let Some(ref dt) = filter.data_type {
					return &s.data_type == dt;
				}
				true
			})
			.collect();

		if sources_to_search.is_empty() {
			return Ok(Vec::new());
		}

		let mut all_results = Vec::new();

		for source_info in &sources_to_search {
			let db = match self.sources.open(&source_info.id).await {
				Ok(db) => db,
				Err(e) => {
					tracing::warn!(source_id = %source_info.id, error = %e, "failed to open source for search");
					continue;
				}
			};

			let temporal = if filter.date_after.is_some() || filter.date_before.is_some() {
				Some(TemporalFilter {
					date_after: filter.date_after.as_deref(),
					date_before: filter.date_before.as_deref(),
				})
			} else {
				None
			};

			let fts_hits = match db.fts_search(query, limit, temporal).await {
				Ok(hits) => hits,
				Err(e) => {
					tracing::debug!(source_id = %source_info.id, error = %e, "FTS search failed");
					Vec::new()
				}
			};

			if fts_hits.is_empty() {
				continue;
			}

			let record_type = db.schema().search.primary_model.clone();
			let external_ids: Vec<String> =
				fts_hits.iter().map(|h| h.external_id.clone()).collect();
			let overlays = db
				.overlays_for(&record_type, &external_ids)
				.await
				.unwrap_or_else(|e| {
					tracing::warn!(source_id = %source_info.id, error = %e, "failed to load overlays");
					HashMap::new()
				});

			for hit in fts_hits {
				let overlay = overlays.get(&hit.external_id).cloned();
				all_results.push(SearchResult {
					id: hit.id,
					external_id: hit.external_id,
					record_type: record_type.clone(),
					title: hit.title,
					preview: hit.preview.unwrap_or_default(),
					subtitle: hit.subtitle,
					snippet: None,
					rank: hit.rank,
					source_id: source_info.id.clone(),
					source_name: source_info.name.clone(),
					data_type: source_info.data_type.clone(),
					data_type_icon: None,
					date: hit.date,
					trust_tier: source_info.trust_tier,
					overlay,
				});
			}
		}

		if filter.sort_by_date {
			all_results.sort_by(|a, b| {
				let da = a.date.as_deref().unwrap_or("");
				let db = b.date.as_deref().unwrap_or("");
				db.cmp(da)
			});
		} else {
			// FTS5 ranks ascending: a more negative score is a better match.
			all_results.sort_by(|a, b| a.rank.total_cmp(&b.rank));
		}
		all_results.truncate(limit);

		Ok(all_results)
	}
}
