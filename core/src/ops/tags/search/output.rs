//! Output for search semantic tags action

use crate::domain::tag::Tag;
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SearchTagsOutput {
	/// Tags found by the search
	pub tags: Vec<TagSearchResult>,

	/// Total number of results found (may be more than returned if limited)
	pub total_found: usize,

	/// Whether results were disambiguated using context
	pub disambiguated: bool,

	/// Search query that was executed
	pub query: String,

	/// Applied filters
	pub filters: SearchFilters,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct TagSearchResult {
	/// The semantic tag
	pub tag: Tag,

	/// Relevance score (0.0-1.0)
	pub relevance: f32,

	/// Which name variant matched the search
	pub matched_variant: Option<String>,

	/// Context score if disambiguation was used
	pub context_score: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SearchFilters {
	pub namespace: Option<String>,
	pub tag_type: Option<String>,
	pub include_archived: bool,
	pub limit: Option<usize>,
}

impl SearchTagsOutput {
	/// Create a successful search output
	pub fn success(
		tags: Vec<Tag>,
		query: String,
		namespace: Option<String>,
		tag_type: Option<String>,
		include_archived: bool,
		limit: Option<usize>,
		disambiguated: bool,
	) -> Self {
		let results: Vec<TagSearchResult> = tags
			.into_iter()
			.enumerate()
			.map(|(i, tag)| TagSearchResult {
				tag,
				relevance: 1.0 - (i as f32 * 0.1), // Simple relevance scoring
				matched_variant: None,
				context_score: None,
			})
			.collect();

		let total_found = results.len();

		Self {
			tags: results,
			total_found,
			disambiguated,
			query,
			filters: SearchFilters {
				namespace,
				tag_type,
				include_archived,
				limit,
			},
		}
	}

	/// Create output with context scores for disambiguation
	pub fn with_context_scores(mut self, context_scores: Vec<f32>) -> Self {
		for (result, score) in self.tags.iter_mut().zip(context_scores.iter()) {
			result.context_score = Some(*score);
			result.relevance = *score;
		}

		// Sort by context score
		self.tags.sort_by(|a, b| {
			b.context_score
				.partial_cmp(&a.context_score)
				.unwrap_or(std::cmp::Ordering::Equal)
		});

		self.disambiguated = true;
		self
	}

	/// Mark which variants matched for each result
	pub fn with_matched_variants(mut self, matched_variants: Vec<Option<String>>) -> Self {
		for (result, variant) in self.tags.iter_mut().zip(matched_variants.iter()) {
			result.matched_variant = variant.clone();
		}
		self
	}
}
