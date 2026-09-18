//! Output for file search operations

use crate::domain::File;
use crate::ops::search::FilterKind;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// Main output structure for file search operations
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct FileSearchOutput {
	/// Flat file array matching DirectoryListingOutput - primary field for explorer
	pub files: Vec<File>,
	/// Search results with scoring metadata - use for search-specific UI (scores, highlights)
	pub results: Vec<FileSearchResult>,
	pub total_found: u64,
	/// False when a participating backend capped its scan, making
	/// `total_found` a floor rather than an exact count.
	pub total_is_exact: bool,
	pub search_id: Uuid,
	pub facets: SearchFacets,
	pub suggestions: Vec<String>,
	pub pagination: PaginationInfo,
	pub execution_time_ms: u64,
	/// Which filters are available for this search type
	pub available_filters: HashSet<FilterKind>,
}

/// Individual search result
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct FileSearchResult {
	pub file: File,
	pub score: f32,
	pub score_breakdown: ScoreBreakdown,
	pub highlights: Vec<TextHighlight>,
	pub matched_content: Option<String>,
}

/// Detailed breakdown of how the score was calculated
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ScoreBreakdown {
	pub temporal_score: f32,
	pub semantic_score: Option<f32>,
	pub metadata_score: f32,
	pub recency_boost: f32,
	pub user_preference_boost: f32,
	pub final_score: f32,
}

/// Text highlighting information
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct TextHighlight {
	pub field: String,
	pub text: String,
	pub start: usize,
	pub end: usize,
}

/// Search facets for filtering UI
#[derive(Debug, Clone, Serialize, Deserialize, Default, Type)]
pub struct SearchFacets {
	pub file_types: HashMap<String, u64>,
	/// Content kinds by their wire name, for the filter panel's kind options.
	pub kinds: HashMap<String, u64>,
	pub tags: HashMap<Uuid, u64>,
	pub date_ranges: HashMap<String, u64>,
	pub size_ranges: HashMap<String, u64>,
}

/// Pagination information
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct PaginationInfo {
	pub current_page: u32,
	pub total_pages: u32,
	pub has_next: bool,
	pub has_previous: bool,
	pub limit: u32,
	pub offset: u32,
}

/// Tag facet with count
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct TagFacetCount {
	pub tag_id: Uuid,
	pub tag_name: String,
	pub count: u64,
}

/// Date range facet with count
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DateRangeFacetCount {
	pub range: String,
	pub count: u64,
}

/// Size range facet with count
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SizeRangeFacetCount {
	pub range: String,
	pub count: u64,
}

impl FileSearchOutput {
	/// Create search output from a page of results
	pub fn from_page(
		page: crate::ops::search::arena_search::SearchPage,
		search_id: Uuid,
		execution_time_ms: u64,
		pagination: &crate::ops::search::input::PaginationOptions,
	) -> Self {
		let crate::ops::search::arena_search::SearchPage {
			results,
			total: total_found,
			facets,
			approximate,
		} = page;
		// The page actually served: the caller's window over the true match
		// count, not a synthetic constant.
		let pagination = PaginationInfo::new(pagination.offset, pagination.limit, total_found);
		let files = results.iter().map(|r| r.file.clone()).collect();

		Self {
			files,
			results,
			total_found,
			total_is_exact: !approximate,
			search_id,
			facets,
			suggestions: Vec::new(),
			pagination,
			execution_time_ms,
			available_filters: HashSet::from([
				FilterKind::FileTypes,
				FilterKind::DateRange,
				FilterKind::SizeRange,
				FilterKind::ContentTypes,
				FilterKind::Hidden,
				FilterKind::Tags,
			]),
		}
	}

	/// Add highlights to results
	pub fn with_highlights(mut self, highlights: HashMap<Uuid, Vec<TextHighlight>>) -> Self {
		for result in &mut self.results {
			if let Some(result_highlights) = highlights.get(&result.file.id) {
				result.highlights = result_highlights.clone();
			}
		}
		self
	}

	/// Add matched content to results
	pub fn with_matched_content(mut self, content: HashMap<Uuid, String>) -> Self {
		for result in &mut self.results {
			if let Some(matched) = content.get(&result.file.id) {
				result.matched_content = Some(matched.clone());
			}
		}
		self
	}
}

impl SearchFacets {
	/// Generate facets from search results
	pub fn from_results(results: &[FileSearchResult]) -> Self {
		let mut facets = Self::default();
		facets.absorb(results);
		facets
	}

	/// Fold one result set into the counts. Called per partition over the
	/// full filtered matches before any narrowing, so a facet count is a
	/// property of the whole match set rather than of one page.
	pub fn absorb(&mut self, results: &[FileSearchResult]) {
		for result in results {
			let file = &result.file;

			if let Some(ref extension) = file.extension {
				*self.file_types.entry(extension.clone()).or_insert(0) += 1;
			}

			// Keyed by the wire spelling, so the filter panel can echo a
			// facet key straight back as a content_types filter value.
			let kind = serde_json::to_value(file.content_kind)
				.ok()
				.and_then(|value| value.as_str().map(str::to_owned))
				.unwrap_or_else(|| "unknown".to_string());
			*self.kinds.entry(kind).or_insert(0) += 1;

			let date_range = Self::categorize_date(file.modified_at);
			*self.date_ranges.entry(date_range).or_insert(0) += 1;

			let size_range = Self::categorize_size(file.size);
			*self.size_ranges.entry(size_range).or_insert(0) += 1;
		}
	}

	/// Categorize date into ranges
	fn categorize_date(date: DateTime<Utc>) -> String {
		let now = Utc::now();
		let diff = now - date;

		if diff.num_days() < 1 {
			"Today".to_string()
		} else if diff.num_days() < 7 {
			"This week".to_string()
		} else if diff.num_days() < 30 {
			"This month".to_string()
		} else if diff.num_days() < 365 {
			"This year".to_string()
		} else {
			"Older".to_string()
		}
	}

	/// Categorize size into ranges
	fn categorize_size(size: u64) -> String {
		if size < 1024 {
			"< 1 KB".to_string()
		} else if size < 1024 * 1024 {
			"1 KB - 1 MB".to_string()
		} else if size < 1024 * 1024 * 1024 {
			"1 MB - 1 GB".to_string()
		} else {
			"> 1 GB".to_string()
		}
	}
}

impl PaginationInfo {
	/// Create pagination info
	pub fn new(offset: u32, limit: u32, total: u64) -> Self {
		let current_page = offset / limit;
		let total_pages = ((total as f64) / (limit as f64)).ceil() as u32;
		let has_next = total_pages > 0 && current_page < total_pages - 1;
		let has_previous = current_page > 0;

		Self {
			current_page,
			total_pages,
			has_next,
			has_previous,
			limit,
			offset,
		}
	}
}

impl ScoreBreakdown {
	/// Create a new score breakdown
	pub fn new(
		temporal_score: f32,
		semantic_score: Option<f32>,
		metadata_score: f32,
		recency_boost: f32,
		user_preference_boost: f32,
	) -> Self {
		let final_score = temporal_score
			+ semantic_score.unwrap_or(0.0)
			+ metadata_score
			+ recency_boost
			+ user_preference_boost;

		Self {
			temporal_score,
			semantic_score,
			metadata_score,
			recency_boost,
			user_preference_boost,
			final_score,
		}
	}
}
