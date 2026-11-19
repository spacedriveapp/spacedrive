//! Output of `search.grep`.

use serde::{Deserialize, Serialize};
use specta::Type;

/// One matching line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct GrepMatch {
	/// The file, relative to the scope, with `/` separators. A name that is
	/// not valid UTF-8 is converted lossily.
	pub path: String,
	/// 1-based.
	pub line_number: u64,
	/// The line without its terminator, converted lossily and cut at 4 KiB.
	pub line: String,
	/// Up to `context_lines` lines before the match, in file order.
	pub before: Vec<String>,
	/// Up to `context_lines` lines after the match, in file order.
	pub after: Vec<String>,
}

/// One page of matches.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
pub struct GrepOutput {
	pub matches: Vec<GrepMatch>,
	/// Files searched for this page.
	pub files_scanned: u64,
	/// Files passed over for this page because they hold binary data or are
	/// larger than `max_file_bytes`.
	pub files_skipped: u64,
	/// The page stopped at `max_results`. More matches may follow.
	pub truncated: bool,
	/// Pass back as `cursor` to read the next page. Set when `truncated` is.
	pub next_cursor: Option<String>,
}
