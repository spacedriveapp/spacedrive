//! Search a source's SQLite store when no arena covers it.
//!
//! One backend answers per source per request: the router in
//! [`super::ephemeral_search`] sends a source here only when its arena
//! cannot answer, and an empty store result is final — nothing retries the
//! other backend. Matching, filters and scoring mirror the arena path so the
//! two backends return the same hits for the same capture.

use crate::domain::{File, SdPath};
use crate::filetype::FileTypeRegistry;
use crate::infra::query::QueryError;
use crate::ops::search::input::{DateField, SearchFilters};
use crate::ops::search::output::{FileSearchResult, ScoreBreakdown};
use std::path::Path;

/// How many title matches one store hydrates per query. The total keeps
/// counting past it, so a capped page still reports what it stands for; the
/// page marks itself approximate when this trips.
const STORE_MATCH_CAP: usize = 50_000;

/// One store's contribution to a search: its filtered results, the exact
/// title-match total, and whether hydration was capped before filtering.
pub struct StorePartition {
	pub results: Vec<FileSearchResult>,
	pub truncated: bool,
}

/// Search one source's store by name, mirroring the arena's semantics:
/// Unicode case-folded substring matching, bundle lensing, and the same
/// filter set failing closed on what the store cannot answer.
pub async fn search_source_store(
	db: &sd_store::SourceDb,
	source_root: &Path,
	device_slug: &str,
	query: &str,
	scope: Option<&Path>,
	filters: &SearchFilters,
	file_type_registry: &FileTypeRegistry,
) -> Result<StorePartition, QueryError> {
	// The arena path returns nothing for an empty library-wide query; the
	// store does the same rather than dumping a source.
	if query.is_empty() {
		return Ok(StorePartition {
			results: Vec::new(),
			truncated: false,
		});
	}

	let matches = sd_store::read::search_titles(db.pool(), query, STORE_MATCH_CAP)
		.await
		.map_err(|e| QueryError::Internal(format!("store search failed: {e}")))?;

	let mut results = Vec::new();
	for entry in &matches.entries {
		let absolute = source_root.join(&entry.relative_path);
		if let Some(scope) = scope {
			if !absolute.starts_with(scope) {
				continue;
			}
		}
		// Bundle internals are lensed out of search here exactly as they are
		// out of the arena's results.
		if crate::ops::indexing::lens::is_bundle_internal(&absolute) {
			continue;
		}
		if !passes_store_filters(entry, filters, &absolute, file_type_registry) {
			continue;
		}

		let sd_path = SdPath::Physical {
			device_slug: device_slug.to_string(),
			path: absolute,
		};
		let file = File::from_store_entry(entry, sd_path);
		let score = super::ephemeral_search::score_match(&file, query);
		results.push(FileSearchResult {
			file,
			score,
			score_breakdown: ScoreBreakdown::new(score, None, 0.0, 0.0, 0.0),
			highlights: Vec::new(),
			matched_content: None,
		});
	}

	Ok(StorePartition {
		results,
		truncated: matches.truncated,
	})
}

/// The arena filter set, judged from a store row. Semantics match
/// `passes_ephemeral_filters` field for field, including failing closed on
/// timestamps the row cannot prove.
fn passes_store_filters(
	entry: &sd_store::FsEntry,
	filters: &SearchFilters,
	absolute: &Path,
	file_type_registry: &FileTypeRegistry,
) -> bool {
	if !filters.include_hidden.unwrap_or(false) && entry.is_hidden {
		return false;
	}

	if let Some(ref types) = filters.file_types {
		let ext = entry.extension.as_deref().unwrap_or("");
		if !types.iter().any(|t| t.eq_ignore_ascii_case(ext)) {
			return false;
		}
	}

	if let Some(ref range) = filters.size_range {
		let size = entry.size.unwrap_or(0).max(0) as u64;
		if let Some(min) = range.min {
			if size < min {
				return false;
			}
		}
		if let Some(max) = range.max {
			if size > max {
				return false;
			}
		}
	}

	if let Some(ref range) = filters.date_range {
		let millis = match range.field {
			DateField::ModifiedAt => entry.mtime_ms,
			DateField::CreatedAt => entry.created_ms,
			DateField::AccessedAt => entry.atime_ms,
			DateField::IndexedAt => None,
		};
		let Some(date) = millis.and_then(chrono::DateTime::from_timestamp_millis) else {
			return false;
		};
		if let Some(start) = range.start {
			if date < start {
				return false;
			}
		}
		if let Some(end) = range.end {
			if date > end {
				return false;
			}
		}
	}

	if let Some(ref content_types) = filters.content_types {
		let identified_kind = file_type_registry.identify_by_extension(absolute);
		if !content_types.contains(&identified_kind) {
			return false;
		}
	}

	true
}
