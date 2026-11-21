//! Input for `search.content`.

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{domain::addressing::SdPath, ops::indexing::text_extract::TextKind};

/// Hits returned per page when the caller does not say.
pub const DEFAULT_MAX_RESULTS: u32 = 20;
/// The largest page a caller may ask for.
pub const MAX_RESULTS_LIMIT: u32 = 200;

/// Search the indexed text of the files under `scope`.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ContentSearchInput {
	/// A directory or file under a source tracked on this device.
	pub scope: SdPath,
	/// Words that must all appear. Text in double quotes is a phrase, and a
	/// word ending in `*` matches as a prefix.
	pub query: String,
	/// Only files read by these extractors. Empty means every kind.
	#[serde(default)]
	pub kinds: Vec<TextKind>,
	/// Hits per page. Defaults to 20.
	#[serde(default)]
	pub max_results: Option<u32>,
	/// The `next_cursor` of the previous page, to continue from there.
	#[serde(default)]
	pub cursor: Option<String>,
}

impl ContentSearchInput {
	pub fn max_results(&self) -> usize {
		self.max_results.unwrap_or(DEFAULT_MAX_RESULTS) as usize
	}

	/// The offset the cursor names.
	pub fn offset(&self) -> Result<usize, String> {
		match &self.cursor {
			None => Ok(0),
			Some(cursor) => cursor
				.parse()
				.map_err(|_| format!("cursor '{cursor}' was not returned by search.content")),
		}
	}

	/// Reject input that could not run or could not be bounded.
	pub fn validate(&self) -> Result<(), String> {
		if sd_store::match_expression(&self.query).is_empty() {
			return Err("query must contain at least one word".into());
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
