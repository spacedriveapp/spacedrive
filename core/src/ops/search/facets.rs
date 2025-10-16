//! Search facets for filtering UI

use super::output::*;
use std::collections::HashMap;
use uuid::Uuid;

/// Facet builder for search results
pub struct FacetBuilder {
	file_types: HashMap<String, u64>,
	tags: HashMap<Uuid, u64>,
	locations: HashMap<Uuid, u64>,
	date_ranges: HashMap<String, u64>,
	size_ranges: HashMap<String, u64>,
}

impl FacetBuilder {
	pub fn new() -> Self {
		Self {
			file_types: HashMap::new(),
			tags: HashMap::new(),
			locations: HashMap::new(),
			date_ranges: HashMap::new(),
			size_ranges: HashMap::new(),
		}
	}

	/// Add a result to the facet counts
	pub fn add_result(&mut self, result: &FileSearchResult) {
		let file = &result.file;

		// Count file types
		if let Some(ref extension) = file.extension {
			*self.file_types.entry(extension.clone()).or_insert(0) += 1;
		}

		// Count date ranges
		let date_range = self.categorize_date(file.modified_at);
		*self.date_ranges.entry(date_range).or_insert(0) += 1;

		// Count size ranges
		let size_range = self.categorize_size(file.size);
		*self.size_ranges.entry(size_range).or_insert(0) += 1;
	}

	/// Build the final facets
	pub fn build(self) -> SearchFacets {
		SearchFacets {
			file_types: self.file_types,
			tags: self.tags,
			locations: self.locations,
			date_ranges: self.date_ranges,
			size_ranges: self.size_ranges,
		}
	}

	/// Categorize date into ranges
	fn categorize_date(&self, date: chrono::DateTime<chrono::Utc>) -> String {
		let now = chrono::Utc::now();
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
	fn categorize_size(&self, size: u64) -> String {
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

impl Default for FacetBuilder {
	fn default() -> Self {
		Self::new()
	}
}

/// Generate search suggestions based on query and results
pub struct SuggestionGenerator {
	query: String,
	results: Vec<FileSearchResult>,
}

impl SuggestionGenerator {
	pub fn new(query: String, results: Vec<FileSearchResult>) -> Self {
		Self { query, results }
	}

	/// Generate suggestions based on the query and results
	pub fn generate(&self) -> Vec<String> {
		let mut suggestions = Vec::new();

		// Add file extension suggestions if query doesn't have one
		if !self.query.contains('.') {
			let extensions: Vec<&str> = self
				.results
				.iter()
				.filter_map(|r| r.file.extension.as_deref())
				.collect();

			let mut extension_counts: HashMap<&str, usize> = HashMap::new();
			for ext in extensions {
				*extension_counts.entry(ext).or_insert(0) += 1;
			}

			// Add most common extensions
			let mut sorted_extensions: Vec<_> = extension_counts.into_iter().collect();
			sorted_extensions.sort_by(|a, b| b.1.cmp(&a.1));

			for (ext, _) in sorted_extensions.into_iter().take(5) {
				suggestions.push(format!("{} .{}", self.query, ext));
			}
		}

		// Add common search patterns
		suggestions.extend([
			format!("{} recent", self.query),
			format!("{} large", self.query),
			format!("{} small", self.query),
			format!("{} today", self.query),
			format!("{} this week", self.query),
		]);

		suggestions
	}
}
