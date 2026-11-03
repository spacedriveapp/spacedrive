//! Query router: fan-out search across sources.
//!
//! Every source is searched through its record table, so results from different data
//! types come back in one shape. Durable overlays are composed onto the hits
//! before they are returned — callers never join the layers themselves.

use std::collections::HashMap;
use std::sync::Arc;

use crate::engine::SourceRef;
use crate::error::Result;
use crate::search::{SearchFilter, SearchResult};
use sd_store::db::TemporalFilter;
use sd_store::source::SourceManager;

const DEFAULT_LIMIT: usize = 20;

/// Routes search queries across all sources.
pub struct SearchRouter {
	pub(crate) sources: Arc<SourceManager>,
}

impl SearchRouter {
	pub fn new(sources: Arc<SourceManager>) -> Self {
		Self { sources }
	}

	/// Search across the given sources, filtered further by the query's own
	/// terms.
	pub async fn search(
		&self,
		query: &str,
		filter: Option<SearchFilter>,
		sources: &[SourceRef],
	) -> Result<Vec<SearchResult>> {
		let filter = filter.unwrap_or_default();
		let limit = filter.limit.unwrap_or(DEFAULT_LIMIT);

		let sources_to_search: Vec<_> = sources
			.iter()
			.filter(|s| {
				filter.source_id.as_ref().is_none_or(|id| &s.id == id)
					&& filter
						.data_type
						.as_ref()
						.is_none_or(|dt| &s.data_type == dt)
			})
			.collect();

		if sources_to_search.is_empty() {
			return Ok(Vec::new());
		}

		let mut all_results = Vec::new();

		for source_info in &sources_to_search {
			let db = self.sources.open(&source_info.id).await.map_err(|e| {
				crate::error::Error::Other(format!(
					"failed to open source {} for search: {e}",
					source_info.id
				))
			})?;

			let temporal = if filter.date_after.is_some() || filter.date_before.is_some() {
				Some(TemporalFilter {
					date_after: filter.date_after.as_deref(),
					date_before: filter.date_before.as_deref(),
				})
			} else {
				None
			};

			let fts_hits = db.fts_search(query, limit, temporal).await.map_err(|e| {
				crate::error::Error::Other(format!(
					"search failed for source {}: {e}",
					source_info.id
				))
			})?;

			if fts_hits.is_empty() {
				continue;
			}

			let record_type = db.schema().search.primary_model.clone();
			let record_uuids: Vec<uuid::Uuid> = fts_hits.iter().map(|h| h.id).collect();
			let overlays = db.overlays_for(&record_uuids).await.unwrap_or_else(|e| {
				tracing::warn!(source_id = %source_info.id, error = %e, "failed to load overlays");
				HashMap::new()
			});

			for hit in fts_hits {
				let overlay = overlays.get(&hit.id).cloned();
				all_results.push(SearchResult {
					id: hit.id.to_string(),
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

#[cfg(test)]
mod tests {
	use super::*;

	fn source() -> SourceRef {
		SourceRef {
			id: "missing".into(),
			name: "Missing".into(),
			data_type: "note".into(),
			adapter_id: "test".into(),
			config: serde_json::Value::Null,
			trust_tier: Default::default(),
		}
	}

	#[tokio::test]
	async fn source_and_type_filters_intersect_and_open_errors_surface() {
		let dir = tempfile::tempdir().unwrap();
		let router = SearchRouter::new(Arc::new(SourceManager::new(dir.path().to_path_buf())));
		let filter = SearchFilter {
			source_id: Some("missing".into()),
			data_type: Some("email".into()),
			..Default::default()
		};
		assert!(router
			.search("test", Some(filter), &[source()])
			.await
			.unwrap()
			.is_empty());
		let filter = SearchFilter {
			source_id: Some("missing".into()),
			data_type: Some("note".into()),
			..Default::default()
		};
		let error = router
			.search("test", Some(filter), &[source()])
			.await
			.unwrap_err()
			.to_string();
		assert!(
			error.contains("missing") && error.contains("failed to open source"),
			"{error}"
		);
	}
	#[tokio::test]
	async fn fts_failure_is_reported_for_a_retained_store() {
		let dir = tempfile::tempdir().unwrap();
		let manager = Arc::new(SourceManager::new(dir.path().to_path_buf()));
		use crate::adapter::{script::ScriptAdapter, Adapter};
		let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../adapters/obsidian");
		let adapter = ScriptAdapter::from_dir(&path).unwrap();
		manager.create("missing", adapter.schema()).await.unwrap();
		let router = SearchRouter::new(manager);
		let error = router
			.search("\"unterminated", None, &[source()])
			.await
			.unwrap_err()
			.to_string();
		assert!(
			error.contains("search failed for source missing"),
			"{error}"
		);
		assert!(router
			.search("needle", None, &[source()])
			.await
			.unwrap()
			.is_empty());
	}
}
