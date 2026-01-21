//! Ephemeral index search implementation
//!
//! This module provides search functionality for the in-memory ephemeral index,
//! enabling search in unindexed locations and external drives.

use crate::domain::{File, SdPath};
use crate::filetype::FileTypeRegistry;
use crate::infra::query::QueryError;
use crate::ops::indexing::database_storage::EntryMetadata;
use crate::ops::indexing::ephemeral::EphemeralIndexCache;
use crate::ops::indexing::state::EntryKind;
use crate::ops::search::input::{DateField, SearchFilters};
use crate::ops::search::output::{FileSearchResult, ScoreBreakdown};
use std::cmp::Ordering;
use std::path::PathBuf;
use uuid::Uuid;

/// Search the ephemeral index for files matching the query
pub async fn search_ephemeral_index(
	query: &str,
	path_scope: &SdPath,
	filters: &SearchFilters,
	cache: &EphemeralIndexCache,
	file_type_registry: &FileTypeRegistry,
) -> Result<Vec<FileSearchResult>, QueryError> {
	// Get local path from SdPath
	let local_path = match path_scope {
		SdPath::Physical { path, .. } => path.clone(),
		_ => {
			return Ok(Vec::new()); // Only physical paths supported for ephemeral
		}
	};

	// Get ephemeral index (use get_for_search to check parent paths)
	let index_arc = cache
		.get_for_search(&local_path)
		.ok_or_else(|| QueryError::Internal("Ephemeral index not found".to_string()))?;

	// Perform name-based search with read lock
	let matching_paths = {
		let index = index_arc.read().await;

		if query.is_empty() {
			// Empty query: return all files in scope
			index.list_directory(&local_path).unwrap_or_default()
		} else {
			// Use registry for substring search
			let query_lower = query.to_lowercase();

			// Try exact name match first
			let mut paths = index.find_by_name(&query_lower);
			tracing::debug!("Exact match for '{}': {} paths", query_lower, paths.len());

			// If no exact matches, try prefix search
			if paths.is_empty() {
				paths = index.find_by_prefix(&query_lower);
				tracing::debug!("Prefix match for '{}': {} paths", query_lower, paths.len());
			}

			// If still no matches, try substring search
			if paths.is_empty() {
				paths = index.find_containing(&query_lower);
				tracing::debug!(
					"Substring match for '{}': {} paths",
					query_lower,
					paths.len()
				);
			}

			tracing::debug!("Total paths before scope filter: {}", paths.len());
			tracing::debug!("Scope path: {:?}", local_path);

			// Filter to only paths within scope
			let filtered: Vec<PathBuf> = paths
				.into_iter()
				.filter(|path| path.starts_with(&local_path))
				.collect();

			tracing::debug!("Paths after scope filter: {}", filtered.len());
			filtered
		}
	};

	tracing::debug!(
		"Converting {} matching paths to results",
		matching_paths.len()
	);

	// Convert to FileSearchResult with lazy UUID assignment
	// Acquire write lock once for the entire batch instead of per-entry
	let mut index = index_arc.write().await;
	let mut results = Vec::new();

	for path in matching_paths {
		if let Some(metadata) = index.get_entry_ref(&path) {
			// Apply filters
			if !passes_ephemeral_filters(&metadata, filters, file_type_registry) {
				continue;
			}

			// Get or assign UUID (lazy generation)
			let uuid = index.get_or_assign_uuid(&path);

			// Build SdPath
			let sd_path = match path_scope {
				SdPath::Physical { device_slug, .. } => SdPath::Physical {
					device_slug: device_slug.clone(),
					path: path.clone(),
				},
				_ => continue,
			};

			// Get content kind
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

	// Sort by score and limit
	results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal));
	results.truncate(200);

	Ok(results)
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

	// Tags and locations are not available in ephemeral
	// These filters are simply ignored for ephemeral searches

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
