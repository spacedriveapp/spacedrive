//! Search result sorting utilities

use super::input::*;
use sea_orm::{ColumnTrait, Order, QueryOrder};

/// Sort builder for search queries
pub struct SortBuilder {
	order: Vec<(String, Order)>,
}

impl SortBuilder {
	pub fn new() -> Self {
		Self { order: Vec::new() }
	}

	pub fn build(self) -> Vec<(String, Order)> {
		self.order
	}

	/// Apply sorting based on sort options
	pub fn apply_sort(mut self, sort: &SortOptions) -> Self {
		let direction = match sort.direction {
			SortDirection::Asc => Order::Asc,
			SortDirection::Desc => Order::Desc,
		};

		match sort.field {
			SortField::Relevance => {
				// For relevance, we'll sort by a combination of factors
				// This is a placeholder - actual relevance scoring would be more complex
				self.order.push(("modified_at".to_string(), Order::Desc));
			}
			SortField::Name => {
				self.order.push(("name".to_string(), direction));
			}
			SortField::Size => {
				self.order.push(("size".to_string(), direction));
			}
			SortField::ModifiedAt => {
				self.order.push(("modified_at".to_string(), direction));
			}
			SortField::CreatedAt => {
				self.order.push(("created_at".to_string(), direction));
			}
			SortField::IndexedAt => {
				self.order.push(("indexed_at".to_string(), direction));
			}
		}

		self
	}

	/// Add secondary sort by name
	pub fn secondary_by_name(mut self) -> Self {
		self.order.push(("name".to_string(), Order::Asc));
		self
	}
}

impl Default for SortBuilder {
	fn default() -> Self {
		Self::new()
	}
}

/// Calculate relevance score for search results
pub struct RelevanceCalculator {
	query: String,
}

impl RelevanceCalculator {
	pub fn new(query: String) -> Self {
		Self { query }
	}

	/// Calculate relevance score for a filename
	pub fn calculate_filename_score(&self, filename: &str) -> f32 {
		let query_lower = self.query.to_lowercase();
		let filename_lower = filename.to_lowercase();

		// Exact match gets highest score
		if filename_lower == query_lower {
			return 1.0;
		}

		// Starts with query gets high score
		if filename_lower.starts_with(&query_lower) {
			return 0.9;
		}

		// Contains query gets medium score
		if filename_lower.contains(&query_lower) {
			return 0.7;
		}

		// Word boundary match gets lower score
		let words: Vec<&str> = filename_lower.split_whitespace().collect();
		let query_words: Vec<&str> = query_lower.split_whitespace().collect();

		let mut score = 0.0;
		for query_word in &query_words {
			for filename_word in &words {
				if filename_word.starts_with(query_word) {
					score += 0.5;
				} else if filename_word.contains(query_word) {
					score += 0.3;
				}
			}
		}

		// Normalize score
		if !query_words.is_empty() {
			score / query_words.len() as f32
		} else {
			0.0
		}
	}

	/// Calculate recency boost based on modification time
	pub fn calculate_recency_boost(&self, modified_at: chrono::DateTime<chrono::Utc>) -> f32 {
		let now = chrono::Utc::now();
		let diff = now - modified_at;

		// Boost decreases over time
		if diff.num_days() < 1 {
			0.2 // Recent files get 20% boost
		} else if diff.num_days() < 7 {
			0.1 // This week gets 10% boost
		} else if diff.num_days() < 30 {
			0.05 // This month gets 5% boost
		} else {
			0.0 // Older files get no boost
		}
	}

	/// Calculate user preference boost (placeholder)
	pub fn calculate_user_preference_boost(&self, _entry_id: i32) -> f32 {
		// TODO: Implement based on user behavior, favorites, etc.
		0.0
	}
}
