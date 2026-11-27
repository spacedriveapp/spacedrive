//! Input for `search.semantic` and `search.semantic.status`.

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{domain::addressing::SdPath, ops::indexing::text_extract::TextKind};

/// Hits returned per page when the caller does not say.
pub const DEFAULT_MAX_RESULTS: u32 = 20;
/// The largest page a caller may ask for.
pub const MAX_RESULTS_LIMIT: u32 = 100;
/// The deepest a cursor may page. Each page re-ranks from the top.
pub const MAX_OFFSET: usize = 500;
/// The longest query embedded, in bytes. A query is a sentence, not a
/// document.
pub const MAX_QUERY_BYTES: usize = 1024;

/// Search the files under `scope` by meaning and words.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SemanticSearchInput {
	/// A directory or file under a source tracked on this device.
	pub scope: SdPath,
	/// What to find, in plain language.
	pub query: String,
	/// Only files of these kinds. Empty means every kind.
	#[serde(default)]
	pub kinds: Vec<TextKind>,
	/// Hits per page. Defaults to 20.
	#[serde(default)]
	pub max_results: Option<u32>,
	/// The `next_cursor` of the previous page.
	#[serde(default)]
	pub cursor: Option<String>,
}

impl SemanticSearchInput {
	pub fn max_results(&self) -> usize {
		self.max_results.unwrap_or(DEFAULT_MAX_RESULTS) as usize
	}

	/// The offset the cursor names.
	pub fn offset(&self) -> Result<usize, String> {
		match &self.cursor {
			None => Ok(0),
			Some(cursor) => cursor
				.parse()
				.ok()
				.filter(|offset| *offset <= MAX_OFFSET)
				.ok_or_else(|| format!("cursor '{cursor}' was not returned by search.semantic")),
		}
	}

	/// Reject input that could not run or could not be bounded.
	pub fn validate(&self) -> Result<(), String> {
		if self.query.trim().is_empty() {
			return Err("query must not be empty".into());
		}
		if self.query.len() > MAX_QUERY_BYTES {
			return Err(format!("query must be at most {MAX_QUERY_BYTES} bytes"));
		}
		if let Some(max) = self.max_results {
			if max == 0 || max > MAX_RESULTS_LIMIT {
				return Err(format!(
					"max_results must be between 1 and {MAX_RESULTS_LIMIT}"
				));
			}
		}
		self.offset().map(|_| ())
	}
}

/// Ask whether semantic search can run for the source holding `scope`.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SemanticStatusInput {
	/// A path under a source tracked on this device.
	pub scope: SdPath,
}

#[cfg(test)]
mod tests {
	use super::*;

	fn input(query: &str) -> SemanticSearchInput {
		SemanticSearchInput {
			scope: SdPath::local("/tmp"),
			query: query.into(),
			kinds: vec![],
			max_results: None,
			cursor: None,
		}
	}

	#[test]
	fn input_is_bounded() {
		assert!(input("kitchen budget").validate().is_ok());
		assert!(input("   ").validate().is_err());
		assert!(input(&"a".repeat(MAX_QUERY_BYTES + 1)).validate().is_err());
		let mut paged = input("x");
		paged.cursor = Some("20".into());
		assert_eq!(paged.offset(), Ok(20));
		paged.cursor = Some("9999".into());
		assert!(paged.validate().is_err());
		paged.cursor = Some("nope".into());
		assert!(paged.validate().is_err());
		let mut big = input("x");
		big.max_results = Some(MAX_RESULTS_LIMIT + 1);
		assert!(big.validate().is_err());
	}
}
