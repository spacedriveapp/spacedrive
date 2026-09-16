//! Search over the volume index.
//!
//! Every drive this machine knows about has a partition in the arena, so a
//! library-wide search is a fan-out over all of them and a scoped one resolves
//! the partition that covers the path. Nothing here reads the durable store:
//! names live in memory, which is what makes a search answer while someone is
//! still typing.

use crate::domain::{File, SdPath};
use crate::filetype::FileTypeRegistry;
use crate::infra::query::QueryError;
use crate::ops::indexing::ephemeral::EphemeralIndexCache;
use crate::ops::indexing::metadata::EntryMetadata;
use crate::ops::indexing::state::EntryKind;
use crate::ops::search::input::{DateField, PaginationOptions, SearchFilters, SortOptions};
use crate::ops::search::output::{FileSearchResult, ScoreBreakdown};
use crate::ops::search::pipeline;
use std::path::PathBuf;
use uuid::Uuid;

/// Search one partition of the volume index, scoped to a path within it.
/// A scope on another device is served from that device's replica.
///
/// Returns the requested page and the true pre-pagination match count.
pub async fn search_ephemeral_index(
	query: &str,
	path_scope: &SdPath,
	filters: &SearchFilters,
	sort: &SortOptions,
	pagination: &PaginationOptions,
	context: &std::sync::Arc<crate::context::CoreContext>,
	cache: &EphemeralIndexCache,
	file_type_registry: &FileTypeRegistry,
) -> Result<(Vec<FileSearchResult>, u64), QueryError> {
	let SdPath::Physical {
		path: local_path,
		device_slug,
	} = path_scope
	else {
		return Ok((Vec::new(), 0));
	};

	if *device_slug != crate::device::get_current_device_slug() {
		let shares = crate::service::mounts::peer::remote_shares().await;
		let Some(share) = shares.iter().find(|share| {
			local_path.starts_with(&share.info.root)
				&& context
					.device_manager
					.get_device_slug(share.device_id)
					.is_some_and(|slug| slug == *device_slug)
		}) else {
			return Ok((Vec::new(), 0));
		};

		let matching_paths = {
			let index = share.index.read().await;
			matches_in(&index, query, Some(local_path))
		};
		let results = collect_results(
			&share.index,
			matching_paths,
			query,
			device_slug,
			filters,
			file_type_registry,
		)
		.await?;
		let total = results.len() as u64;
		return Ok((pipeline::page(results, sort, pagination), total));
	}

	// The volume decides how the scope is written: the index holds the
	// volume's spelling, so a scope reached through an alias such as
	// /Users/me has to be rewritten before it can select a partition or
	// filter its paths.
	let local_path = match context.volume_manager.locate_path(local_path).await {
		Some((_, spelled)) => spelled,
		None => local_path.clone(),
	};

	// A registered source that has not been touched this session restores from
	// its snapshot here, including detached drives, whose indexes serve
	// read-only.
	cache.ensure_restored(&local_path).await;

	let Some(index_arc) = cache.get_for_search(&local_path) else {
		return Ok((Vec::new(), 0));
	};

	let matching_paths = {
		let index = index_arc.read().await;
		matches_in(&index, query, Some(&local_path))
	};

	let results = collect_results(
		&index_arc,
		matching_paths,
		query,
		device_slug,
		filters,
		file_type_registry,
	)
	.await?;
	let total = results.len() as u64;
	Ok((pipeline::page(results, sort, pagination), total))
}

/// Search every partition, which is what a library-wide search is now that
/// every attached drive is mapped — including paired devices' sources,
/// whose replicated indexes answer from memory just like the local ones.
/// Returns the requested page and the true pre-pagination match count. Each
/// partition contributes its full filtered match count to the total and only
/// its page-window of candidates to the merge.
pub async fn search_every_index(
	query: &str,
	filters: &SearchFilters,
	sort: &SortOptions,
	pagination: &PaginationOptions,
	context: &std::sync::Arc<crate::context::CoreContext>,
	cache: &EphemeralIndexCache,
	file_type_registry: &FileTypeRegistry,
) -> Result<(Vec<FileSearchResult>, u64), QueryError> {
	let mut candidates = Vec::new();
	let mut total: u64 = 0;
	let window = pipeline::window(pagination);
	let local_slug = crate::device::get_current_device_slug();

	for index_arc in cache.all_indexes() {
		let matching_paths = {
			let index = index_arc.read().await;
			matches_in(&index, query, None)
		};

		let mut partition = collect_results(
			&index_arc,
			matching_paths,
			query,
			&local_slug,
			filters,
			file_type_registry,
		)
		.await?;
		total += partition.len() as u64;
		pipeline::narrow(&mut partition, sort, window);
		candidates.extend(partition);
	}

	for share in crate::service::mounts::peer::remote_shares().await {
		// A hit is addressed by its owning device's slug, which is what
		// routes a listing or preview of it back through the replica.
		let Some(slug) = context.device_manager.get_device_slug(share.device_id) else {
			tracing::debug!(
				"replica of {} has no slug for device {}; skipping in search",
				share.info.root.display(),
				share.device_id
			);
			continue;
		};

		let matching_paths = {
			let index = share.index.read().await;
			matches_in(&index, query, None)
		};

		let mut partition = collect_results(
			&share.index,
			matching_paths,
			query,
			&slug,
			filters,
			file_type_registry,
		)
		.await?;
		total += partition.len() as u64;
		pipeline::narrow(&mut partition, sort, window);
		candidates.extend(partition);
	}

	Ok((pipeline::page(candidates, sort, pagination), total))
}

/// Paths in one partition whose name contains the query, narrowed to a scope
/// if given.
///
/// Substring matching subsumes exact and prefix hits, and scoring already
/// ranks them above it, so an exact query surfaces its file first without
/// hiding everything else that contains the term.
fn matches_in(
	index: &crate::ops::indexing::ephemeral::EphemeralIndex,
	query: &str,
	scope: Option<&PathBuf>,
) -> Vec<PathBuf> {
	if query.is_empty() {
		return match scope {
			Some(path) => index.list_directory(path).unwrap_or_default(),
			None => Vec::new(),
		};
	}

	let query = query.to_lowercase();
	let paths = index.find_containing(&query);

	match scope {
		Some(root) => paths
			.into_iter()
			.filter(|path| path.starts_with(root))
			.collect(),
		None => paths,
	}
}

async fn collect_results(
	index_arc: &std::sync::Arc<
		tokio::sync::RwLock<crate::ops::indexing::ephemeral::EphemeralIndex>,
	>,
	matching_paths: Vec<PathBuf>,
	query: &str,
	device_slug: &str,
	filters: &SearchFilters,
	file_type_registry: &FileTypeRegistry,
) -> Result<Vec<FileSearchResult>, QueryError> {
	// One write lock for the batch rather than one per entry: the lazy uuid
	// assignment needs it and the lock is the expensive part.
	let mut index = index_arc.write().await;
	let mut results = Vec::new();

	for path in matching_paths {
		if let Some(metadata) = index.get_entry_ref(&path) {
			// Bundle internals are lensed out of search; their contents
			// surface through the source that describes them.
			if crate::ops::indexing::lens::is_bundle_internal(&path) {
				continue;
			}

			// Apply filters
			if !passes_ephemeral_filters(&metadata, filters, file_type_registry) {
				continue;
			}

			// Get or assign UUID (lazy generation)
			let uuid = index.get_or_assign_uuid(&path);

			let sd_path = SdPath::Physical {
				device_slug: device_slug.to_string(),
				path: path.clone(),
			};

			let content_kind = index.get_content_kind(&path);

			// Convert to File
			let mut file = File::from_ephemeral(uuid, &metadata, sd_path);
			file.content_kind = content_kind;

			// Score by relevance
			let score = score_match(&file, query);

			results.push(FileSearchResult {
				file,
				score,
				score_breakdown: ScoreBreakdown::new(score, None, 0.0, 0.0, 0.0),
				highlights: Vec::new(),
				matched_content: None,
			});
		}
	}

	Ok(results)
}

/// Check if metadata passes ephemeral filters
fn passes_ephemeral_filters(
	metadata: &EntryMetadata,
	filters: &SearchFilters,
	file_type_registry: &FileTypeRegistry,
) -> bool {
	// Hidden files are excluded unless asked for. The predicate judges the
	// entry's own name, so searching inside a hidden folder still finds its
	// visible contents.
	if !filters.include_hidden.unwrap_or(false) && metadata.is_hidden {
		return false;
	}

	// File type filter (extension). Case-folded on both sides: the UI shows
	// lowercased extensions while the filesystem stores whatever casing the
	// file was written with.
	if let Some(ref types) = filters.file_types {
		let ext = metadata
			.path
			.extension()
			.and_then(|e| e.to_str())
			.unwrap_or("");
		if !types.iter().any(|t| t.eq_ignore_ascii_case(ext)) {
			return false;
		}
	}

	// Size filter
	if let Some(ref range) = filters.size_range {
		let size = metadata.size;
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

	// Date filter. A missing timestamp fails the filter rather than passing
	// it: an entry that cannot prove it is in the range is not in the range.
	// The index holds no access or indexed-at times, so those fields fail
	// closed until a backend can answer them; the UI offers only the fields
	// advertised as answerable.
	if let Some(ref range) = filters.date_range {
		use chrono::{DateTime, Utc};

		let system_time_opt = match range.field {
			DateField::ModifiedAt => metadata.modified,
			DateField::CreatedAt => metadata.created,
			DateField::AccessedAt => metadata.accessed,
			DateField::IndexedAt => None,
		};

		let Some(system_time) = system_time_opt else {
			return false;
		};
		let date = DateTime::<Utc>::from(system_time);

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

	// Content type filter (via extension using FileTypeRegistry)
	if let Some(ref content_types) = filters.content_types {
		// Use FileTypeRegistry to identify content kind by extension
		let identified_kind = file_type_registry.identify_by_extension(&metadata.path);

		// Check if the identified kind matches any of the requested types
		if !content_types.contains(&identified_kind) {
			return false;
		}
	}

	// Tags have no arena representation, so a tag filter is ignored rather
	// than silently excluding everything.

	true
}

/// Score a match based on query relevance
fn score_match(file: &File, query: &str) -> f32 {
	if query.is_empty() {
		return 0.5; // Neutral score for empty queries
	}

	let name = file.name.to_lowercase();
	let query_lower = query.to_lowercase();

	// Exact match
	if name == query_lower {
		return 1.0;
	}

	// Prefix match (file starts with query)
	if name.starts_with(&query_lower) {
		return 0.9;
	}

	// Word boundary match (query matches a complete word)
	if let Some(base_name) = file.name.split('.').next() {
		if base_name.to_lowercase() == query_lower {
			return 0.85;
		}
	}

	// Contains match (query anywhere in filename)
	if name.contains(&query_lower) {
		// Score higher if query is near the beginning
		if let Some(pos) = name.find(&query_lower) {
			let pos_score = 1.0 - (pos as f32 / name.len() as f32);
			return 0.5 + (pos_score * 0.3);
		}
		return 0.5;
	}

	// Weak match (shouldn't happen with find_containing, but just in case)
	0.1
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::ops::search::input::{DateRangeFilter, SizeRangeFilter};

	fn metadata(name: &str, size: u64, modified: Option<std::time::SystemTime>) -> EntryMetadata {
		EntryMetadata {
			path: std::path::PathBuf::from(format!("/vol/{name}")),
			kind: EntryKind::File,
			size,
			modified,
			accessed: None,
			created: None,
			inode: None,
			permissions: None,
			uid: None,
			gid: None,
			link_target: None,
			is_hidden: name.starts_with('.'),
		}
	}

	fn passes(metadata: &EntryMetadata, filters: &SearchFilters) -> bool {
		let registry = FileTypeRegistry::new();
		passes_ephemeral_filters(metadata, filters, &registry)
	}

	/// Hidden entries are excluded by default and included on request; the
	/// data was always in the index, the filter just never read it.
	#[test]
	fn hidden_entries_are_excluded_unless_asked_for() {
		let dotfile = metadata(".env", 1, None);
		let plain = metadata("env.txt", 1, None);

		let defaults = SearchFilters::default();
		assert!(!passes(&dotfile, &defaults));
		assert!(passes(&plain, &defaults));

		let include = SearchFilters {
			include_hidden: Some(true),
			..Default::default()
		};
		assert!(passes(&dotfile, &include));
	}

	/// The UI shows lowercased extensions; the filesystem stores any casing.
	/// The filter has to meet in the middle.
	#[test]
	fn extension_filter_is_case_insensitive() {
		let upper = metadata("PHOTO.JPG", 1, None);
		let filters = SearchFilters {
			file_types: Some(vec!["jpg".to_string()]),
			..Default::default()
		};
		assert!(passes(&upper, &filters));

		let other = metadata("notes.txt", 1, None);
		assert!(!passes(&other, &filters));
	}

	/// An entry that cannot prove it is in a date range is not in the range.
	#[test]
	fn a_missing_timestamp_fails_a_date_filter() {
		let dated = metadata(
			"dated.txt",
			1,
			Some(std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_600_000_000)),
		);
		let undated = metadata("undated.txt", 1, None);

		let filters = SearchFilters {
			date_range: Some(DateRangeFilter {
				field: DateField::ModifiedAt,
				start: Some(chrono::DateTime::from_timestamp(1_500_000_000, 0).unwrap()),
				end: None,
			}),
			..Default::default()
		};
		assert!(passes(&dated, &filters));
		assert!(!passes(&undated, &filters));
	}

	#[test]
	fn size_bounds_are_inclusive() {
		let file = metadata("mid.bin", 100, None);
		let filters = SearchFilters {
			size_range: Some(SizeRangeFilter {
				min: Some(100),
				max: Some(100),
			}),
			..Default::default()
		};
		assert!(passes(&file, &filters));
	}
}
