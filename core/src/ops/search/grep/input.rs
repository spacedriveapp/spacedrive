//! Input for `search.grep`.

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::domain::addressing::SdPath;

/// Matches returned per page when the caller does not say.
pub const DEFAULT_MAX_RESULTS: u32 = 100;
/// The largest page a caller may ask for.
pub const MAX_RESULTS_LIMIT: u32 = 10_000;
/// Text read per file when the caller does not say. Larger files are skipped.
pub const DEFAULT_MAX_FILE_BYTES: u64 = 1024 * 1024;
/// The most context lines a caller may ask for on each side of a match.
pub const MAX_CONTEXT_LINES: u32 = 20;

/// What to search for and how, independent of where.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GrepOptions {
	/// A regular expression, or a literal string when `literal` is set.
	pub pattern: String,
	/// Match `pattern` as plain text rather than as a regular expression.
	#[serde(default)]
	pub literal: bool,
	/// Ignore case when matching.
	#[serde(default)]
	pub case_insensitive: bool,
	/// Globs a file must match to be searched, such as `*.rs`. Empty means
	/// every file.
	#[serde(default)]
	pub include: Vec<String>,
	/// Globs that exclude files and directories from the search.
	#[serde(default)]
	pub exclude: Vec<String>,
	/// Skip what `.gitignore`, `.ignore` and git's exclude files ignore.
	#[serde(default = "default_true")]
	pub respect_gitignore: bool,
	/// Search hidden files and directories, those whose names start with a dot.
	#[serde(default)]
	pub include_hidden: bool,
	/// Lines of context returned before and after each match.
	#[serde(default)]
	pub context_lines: u32,
	/// Matches per page. Defaults to 100.
	#[serde(default)]
	pub max_results: Option<u32>,
	/// The `next_cursor` of the previous page, to continue from there.
	#[serde(default)]
	pub cursor: Option<String>,
	/// Files larger than this many bytes are skipped. Defaults to 1 MiB.
	#[serde(default)]
	pub max_file_bytes: Option<u64>,
}

fn default_true() -> bool {
	true
}

impl GrepOptions {
	/// A regex search with every other option at its default.
	pub fn new(pattern: impl Into<String>) -> Self {
		Self {
			pattern: pattern.into(),
			literal: false,
			case_insensitive: false,
			include: Vec::new(),
			exclude: Vec::new(),
			respect_gitignore: true,
			include_hidden: false,
			context_lines: 0,
			max_results: None,
			cursor: None,
			max_file_bytes: None,
		}
	}

	pub fn max_results(&self) -> usize {
		self.max_results.unwrap_or(DEFAULT_MAX_RESULTS) as usize
	}

	pub fn max_file_bytes(&self) -> u64 {
		self.max_file_bytes.unwrap_or(DEFAULT_MAX_FILE_BYTES)
	}

	/// Reject input that could not run or could not be bounded.
	pub fn validate(&self) -> Result<(), String> {
		if self.pattern.is_empty() {
			return Err("pattern cannot be empty".into());
		}
		if let Some(max) = self.max_results {
			if max == 0 || max > MAX_RESULTS_LIMIT {
				return Err(format!(
					"max_results must be between 1 and {MAX_RESULTS_LIMIT}"
				));
			}
		}
		if self.context_lines > MAX_CONTEXT_LINES {
			return Err(format!("context_lines cannot exceed {MAX_CONTEXT_LINES}"));
		}
		if self.max_file_bytes == Some(0) {
			return Err("max_file_bytes must be greater than zero".into());
		}
		Ok(())
	}
}

/// Search the files under `scope` for `pattern`.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GrepInput {
	/// A directory or file under a source tracked on this device.
	pub scope: SdPath,
	#[serde(flatten)]
	pub options: GrepOptions,
}
