//! Output for file search operations

use crate::domain::File;
use crate::ops::search::{FilterKind, IndexType};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// Main output structure for file search operations
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct FileSearchOutput {
	pub results: Vec<FileSearchResult>,
	pub total_found: u64,
	pub search_id: Uuid,
	pub facets: SearchFacets,
	pub suggestions: Vec<String>,
	pub pagination: PaginationInfo,
	pub execution_time_ms: u64,
	/// Which index type was used for this search
	pub index_type: IndexType,
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
	pub tags: HashMap<Uuid, u64>,
	pub locations: HashMap<Uuid, u64>,
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

/// Location facet with count
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LocationFacetCount {
	pub location_id: Uuid,
	pub location_name: String,
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
	/// Create a successful search output (defaults to persistent index)
	pub fn success(
		results: Vec<FileSearchResult>,
		total_found: u64,
		search_id: Uuid,
		execution_time_ms: u64,
	) -> Self {
		let facets = SearchFacets::from_results(&results);
		let pagination = PaginationInfo::new(0, 50, total_found);

		Self {
			results,
			total_found,
			search_id,
			facets,
			suggestions: Vec::new(),
			pagination,
			execution_time_ms,
			index_type: IndexType::Persistent,
			available_filters: HashSet::from([
				FilterKind::FileTypes,
				FilterKind::DateRange,
				FilterKind::SizeRange,
				FilterKind::ContentTypes,
				FilterKind::Tags,
				FilterKind::Locations,
			]),
		}
	}

	/// Create search output for ephemeral index results
	pub fn new_ephemeral(
		results: Vec<FileSearchResult>,
		total_found: u64,
		search_id: Uuid,
		execution_time_ms: u64,
	) -> Self {
		let facets = SearchFacets::from_results(&results);
		let pagination = PaginationInfo::new(0, 200, total_found);

		Self {
			results,
			total_found,
			search_id,
			facets,
			suggestions: Vec::new(),
			pagination,
			execution_time_ms,
			index_type: IndexType::Ephemeral,
			available_filters: HashSet::from([
				FilterKind::FileTypes,
				FilterKind::DateRange,
				FilterKind::SizeRange,
				FilterKind::ContentTypes,
			]),
		}
	}

	/// Create search output for persistent index results
	pub fn new_persistent(
		results: Vec<FileSearchResult>,
		total_found: u64,
		search_id: Uuid,
		execution_time_ms: u64,
	) -> Self {
		let facets = SearchFacets::from_results(&results);
		let pagination = PaginationInfo::new(0, 1000, total_found);

		Self {
			results,
			total_found,
			search_id,
			facets,
			suggestions: Vec::new(),
			pagination,
			execution_time_ms,
			index_type: IndexType::Persistent,
			available_filters: HashSet::from([
				FilterKind::FileTypes,
				FilterKind::DateRange,
				FilterKind::SizeRange,
				FilterKind::ContentTypes,
				FilterKind::Tags,
				FilterKind::Locations,
			]),
		}
	}

	/// Create an empty search output
	pub fn empty(query: &str) -> Self {
		Self {
			results: Vec::new(),
			total_found: 0,
			search_id: Uuid::new_v4(),
			facets: SearchFacets::default(),
			suggestions: Self::generate_suggestions(query),
			pagination: PaginationInfo::new(0, 50, 0),
			execution_time_ms: 0,
			index_type: IndexType::Persistent,
			available_filters: HashSet::new(),
		}
	}

	/// Generate search suggestions based on query
	fn generate_suggestions(query: &str) -> Vec<String> {
		let mut suggestions = Vec::new();

		// Add common file extensions if query doesn't have one
		if !query.contains('.') {
			suggestions.extend([
				format!("{} .pdf", query),
				format!("{} .jpg", query),
				format!("{} .mp4", query),
				format!("{} .txt", query),
			]);
		}

		// Add common search patterns
		suggestions.extend([
			format!("{} recent", query),
			format!("{} large", query),
			format!("{} small", query),
		]);

		suggestions
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
		let mut file_types = HashMap::new();
		let mut tags = HashMap::new();
		let mut locations = HashMap::new();
		let mut date_ranges = HashMap::new();
		let mut size_ranges = HashMap::new();

		for result in results {
			let file = &result.file;

			// Count file types
			if let Some(ref extension) = file.extension {
				*file_types.entry(extension.clone()).or_insert(0) += 1;
			}

			// Count date ranges
			let modified_at = file.modified_at;
			let date_range = Self::categorize_date(modified_at);
			*date_ranges.entry(date_range).or_insert(0) += 1;

			// Count size ranges
			let size = file.size;
			let size_range = Self::categorize_size(size);
			*size_ranges.entry(size_range).or_insert(0) += 1;
		}

		Self {
			file_types,
			tags,
			locations,
			date_ranges,
			size_ranges,
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

// ============================================================================
// File-based search output (new enhanced version)
// ============================================================================

/// Enhanced search output that returns File objects instead of Entry objects
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct EnhancedFileSearchOutput {
	pub results: Vec<EnhancedFileSearchResult>,
	pub total_found: u64,
	pub search_id: Uuid,
	pub facets: SearchFacets,
	pub suggestions: Vec<String>,
	pub pagination: PaginationInfo,
	pub execution_time_ms: u64,
	pub index_type: IndexType,
	pub available_filters: HashSet<FilterKind>,
}

/// Enhanced search result with File object
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct EnhancedFileSearchResult {
	pub file: File,
	pub score: f32,
	pub score_breakdown: ScoreBreakdown,
	pub highlights: Vec<TextHighlight>,
	pub matched_content: Option<String>,
}

impl EnhancedFileSearchOutput {
	/// Create a successful search output
	pub fn success(
		results: Vec<EnhancedFileSearchResult>,
		total_found: u64,
		search_id: Uuid,
		execution_time_ms: u64,
	) -> Self {
		Self {
			results,
			total_found,
			search_id,
			facets: SearchFacets::default(),
			suggestions: Vec::new(),
			pagination: PaginationInfo {
				current_page: 1,
				total_pages: 1,
				has_next: false,
				has_previous: false,
				limit: 50,
				offset: 0,
			},
			execution_time_ms,
			index_type: IndexType::Persistent,
			available_filters: HashSet::from([
				FilterKind::FileTypes,
				FilterKind::DateRange,
				FilterKind::SizeRange,
				FilterKind::ContentTypes,
				FilterKind::Tags,
				FilterKind::Locations,
			]),
		}
	}

	/// Convert from the legacy Entry-based output
	pub fn from_legacy_output(
		legacy_output: FileSearchOutput,
		files: Vec<File>,
	) -> Result<Self, String> {
		if legacy_output.results.len() != files.len() {
			return Err(format!(
				"Mismatch between search results ({}) and files ({})",
				legacy_output.results.len(),
				files.len()
			));
		}

		let enhanced_results = legacy_output
			.results
			.into_iter()
			.zip(files.into_iter())
			.map(|(legacy_result, file)| EnhancedFileSearchResult {
				file,
				score: legacy_result.score,
				score_breakdown: legacy_result.score_breakdown,
				highlights: legacy_result.highlights,
				matched_content: legacy_result.matched_content,
			})
			.collect();

		Ok(Self {
			results: enhanced_results,
			total_found: legacy_output.total_found,
			search_id: legacy_output.search_id,
			facets: legacy_output.facets,
			suggestions: legacy_output.suggestions,
			pagination: legacy_output.pagination,
			execution_time_ms: legacy_output.execution_time_ms,
			index_type: legacy_output.index_type,
			available_filters: legacy_output.available_filters,
		})
	}
}
