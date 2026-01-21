//! Input for file search operations

use crate::domain::{ContentKind, SdPath};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;
use uuid::Uuid;

/// Main input structure for file search operations
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct FileSearchInput {
	/// Primary search query (filename, content, or natural language)
	pub query: String,

	/// Search scope (library, location, or specific path)
	pub scope: SearchScope,

	/// Search mode (fast, normal, full)
	pub mode: SearchMode,

	/// Filters to narrow results
	pub filters: SearchFilters,

	/// Sorting options
	pub sort: SortOptions,

	/// Pagination
	pub pagination: PaginationOptions,
}

/// Defines the scope of the filesystem to search within
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub enum SearchScope {
	/// Search the entire library (default)
	Library,
	/// Restrict search to a specific location by its ID
	Location { location_id: Uuid },
	/// Restrict search to a specific directory path and all its descendants
	Path { path: SdPath },
}

/// Defines the search mode and performance characteristics
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub enum SearchMode {
	/// Fast, metadata-only search (<10ms)
	Fast,
	/// Normal search with semantic ranking (<100ms)
	Normal,
	/// Full search with content analysis (<500ms)
	Full,
}

/// Container for all structured filters
#[derive(Debug, Clone, Serialize, Deserialize, Default, Type)]
pub struct SearchFilters {
	pub file_types: Option<Vec<String>>,
	pub tags: Option<TagFilter>,
	pub date_range: Option<DateRangeFilter>,
	pub size_range: Option<SizeRangeFilter>,
	pub locations: Option<Vec<Uuid>>,
	pub content_types: Option<Vec<ContentKind>>,
	pub include_hidden: Option<bool>,
	pub include_archived: Option<bool>,
}

/// Filter for tags, supporting complex boolean logic
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct TagFilter {
	/// Must have all of these tag IDs
	pub include: Vec<Uuid>,
	/// Must not have any of these tag IDs
	pub exclude: Vec<Uuid>,
}

/// Filter for a time-based field
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DateRangeFilter {
	pub field: DateField,
	pub start: Option<DateTime<Utc>>,
	pub end: Option<DateTime<Utc>>,
}

/// Time-based fields that can be filtered
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub enum DateField {
	CreatedAt,
	ModifiedAt,
	AccessedAt,
	IndexedAt,
}

/// Filter for file size in bytes
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SizeRangeFilter {
	pub min: Option<u64>,
	pub max: Option<u64>,
}

/// Sorting options for search results
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SortOptions {
	pub field: SortField,
	pub direction: SortDirection,
}

/// Fields that can be used for sorting
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub enum SortField {
	Relevance,
	Name,
	Size,
	ModifiedAt,
	CreatedAt,
	IndexedAt,
}

/// Sort direction
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub enum SortDirection {
	Asc,
	Desc,
}

/// Pagination options
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct PaginationOptions {
	pub limit: u32,
	pub offset: u32,
}

impl FileSearchInput {
	/// Create a simple search input with default options
	pub fn simple(query: String) -> Self {
		Self {
			query,
			scope: SearchScope::Library,
			mode: SearchMode::Normal,
			filters: SearchFilters::default(),
			sort: SortOptions {
				field: SortField::Relevance,
				direction: SortDirection::Desc,
			},
			pagination: PaginationOptions {
				limit: 50,
				offset: 0,
			},
		}
	}

	/// Create a fast search input for quick results
	pub fn fast(query: String) -> Self {
		Self {
			query,
			scope: SearchScope::Library,
			mode: SearchMode::Fast,
			filters: SearchFilters::default(),
			sort: SortOptions {
				field: SortField::Relevance,
				direction: SortDirection::Desc,
			},
			pagination: PaginationOptions {
				limit: 20,
				offset: 0,
			},
		}
	}

	/// Create a comprehensive search input
	pub fn comprehensive(query: String) -> Self {
		Self {
			query,
			scope: SearchScope::Library,
			mode: SearchMode::Full,
			filters: SearchFilters::default(),
			sort: SortOptions {
				field: SortField::Relevance,
				direction: SortDirection::Desc,
			},
			pagination: PaginationOptions {
				limit: 100,
				offset: 0,
			},
		}
	}

	/// Validate the search input
	pub fn validate(&self) -> Result<(), String> {
		// Allow empty queries when sorting by IndexedAt (for recents view)
		let is_recents_query = self.query.trim().is_empty()
			&& matches!(self.sort.field, SortField::IndexedAt);

		if self.query.trim().is_empty() && !is_recents_query {
			return Err("Query cannot be empty".to_string());
		}

		if self.query.len() > 1000 {
			return Err("Query cannot exceed 1000 characters".to_string());
		}

		if self.pagination.limit == 0 {
			return Err("Pagination limit must be greater than 0".to_string());
		}

		if self.pagination.limit > 1000 {
			return Err("Pagination limit cannot exceed 1000".to_string());
		}

		// Validate date range if provided
		if let Some(date_range) = &self.filters.date_range {
			if let (Some(start), Some(end)) = (date_range.start, date_range.end) {
				if start > end {
					return Err("Date range start must be before end".to_string());
				}
			}
		}

		// Validate size range if provided
		if let Some(size_range) = &self.filters.size_range {
			if let (Some(min), Some(max)) = (size_range.min, size_range.max) {
				if min > max {
					return Err("Size range min must be less than max".to_string());
				}
			}
		}

		Ok(())
	}
}

impl Default for SearchScope {
	fn default() -> Self {
		SearchScope::Library
	}
}

impl Default for SearchMode {
	fn default() -> Self {
		SearchMode::Normal
	}
}

impl Default for SortOptions {
	fn default() -> Self {
		Self {
			field: SortField::Relevance,
			direction: SortDirection::Desc,
		}
	}
}

impl Default for PaginationOptions {
	fn default() -> Self {
		Self {
			limit: 50,
			offset: 0,
		}
	}
}
