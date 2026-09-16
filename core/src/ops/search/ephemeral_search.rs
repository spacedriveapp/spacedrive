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
use crate::ops::search::input::{DateField, SearchFilters};
use crate::ops::search::output::{FileSearchResult, ScoreBreakdown};
use std::cmp::Ordering;
use std::path::PathBuf;
use uuid::Uuid;

/// Search one partition of the volume index, scoped to a path within it.
/// A scope on another device is served from that device's replica.
pub async fn search_ephemeral_index(
	query: &str,
	path_scope: &SdPath,
	filters: &SearchFilters,
	context: &std::sync::Arc<crate::context::CoreContext>,
	cache: &EphemeralIndexCache,
	file_type_registry: &FileTypeRegistry,
) -> Result<Vec<FileSearchResult>, QueryError> {
	let SdPath::Physical {
		path: local_path,
		device_slug,
	} = path_scope
	else {
		return Ok(Vec::new());
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
			return Ok(Vec::new());
		};

		let matching_paths = {
			let index = share.index.read().await;
			matches_in(&index, query, Some(local_path))
		};
		return collect_results(
			&share.index,
			matching_paths,
			query,
			device_slug,
			filters,
			file_type_registry,
		)
		.await;
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
		return Ok(Vec::new());
	};

	let matching_paths = {
		let index = index_arc.read().await;
		matches_in(&index, query, Some(&local_path))
	};

	collect_results(
		&index_arc,
		matching_paths,
		query,
		device_slug,
		filters,
		file_type_registry,
	)
	.await
}

/// Search every partition, which is what a library-wide search is now that
/// every attached drive is mapped — including paired devices' sources,
/// whose replicated indexes answer from memory just like the local ones.
pub async fn search_every_index(
	query: &str,
	filters: &SearchFilters,
	context: &std::sync::Arc<crate::context::CoreContext>,
	cache: &EphemeralIndexCache,
	file_type_registry: &FileTypeRegistry,
) -> Result<Vec<FileSearchResult>, QueryError> {
	let mut results = Vec::new();
	let local_slug = crate::device::get_current_device_slug();

	for index_arc in cache.all_indexes() {
		let matching_paths = {
			let index = index_arc.read().await;
			matches_in(&index, query, None)
		};

		results.extend(
			collect_results(
				&index_arc,
				matching_paths,
				query,
				&local_slug,
				filters,
				file_type_registry,
			)
			.await?,
		);
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

		results.extend(
			collect_results(
				&share.index,
				matching_paths,
				query,
				&slug,
				filters,
				file_type_registry,
			)
			.await?,
		);
	}

	rank(&mut results);
	Ok(results)
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

	rank(&mut results);
	Ok(results)
}

/// Best match first, capped at what a person will scroll through.
fn rank(results: &mut Vec<FileSearchResult>) {
	results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal));
	results.truncate(200);
}

/// Check if metadata passes ephemeral filters
fn passes_ephemeral_filters(
	metadata: &EntryMetadata,
	filters: &SearchFilters,
	file_type_registry: &FileTypeRegistry,
) -> bool {
	// File type filter (extension)
	if let Some(ref types) = filters.file_types {
		let ext = metadata
			.path
			.extension()
			.and_then(|e| e.to_str())
			.unwrap_or("");
		if !types.contains(&ext.to_string()) {
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

	// Date filter
	if let Some(ref range) = filters.date_range {
		use chrono::{DateTime, Utc};

		let system_time_opt = match range.field {
			DateField::ModifiedAt => metadata.modified,
			DateField::CreatedAt => metadata.created,
			DateField::AccessedAt => metadata.accessed,
			DateField::IndexedAt => None, // Ephemeral search doesn't have indexed_at
		};

		if let Some(system_time) = system_time_opt {
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
