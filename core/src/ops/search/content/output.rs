//! Output of `search.content`.

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::ops::indexing::text_extract::TextKind;

/// A piece of a snippet: either text that matched the query or the text
/// around it. Joined in order, the pieces are the snippet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SnippetFragment {
	pub text: String,
	pub matched: bool,
}

/// One ranked file.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ContentSearchHit {
	/// The file, relative to the scope, with `/` separators. The file's
	/// name when the scope is the file itself.
	pub path: String,
	/// The extractor that read it.
	pub kind: TextKind,
	/// BM25 relevance. Larger is better; only the order within one response
	/// means anything.
	pub score: f64,
	/// Up to about 24 words around the best match.
	pub snippet: Vec<SnippetFragment>,
	/// The file holds more than 1 MiB of text and only the start was
	/// indexed, so a match later in the file is not found.
	pub truncated: bool,
	/// Where in the recording the match is: when the words were said, for a
	/// `transcript` hit, or the shot's in and out points, for a `scene`
	/// hit. One recording can answer with several hits, one per moment.
	pub time: Option<SpokenTime>,
}

/// A stretch of a recording, in seconds from its start.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Type)]
pub struct SpokenTime {
	pub start_seconds: f64,
	pub end_seconds: f64,
}

impl SpokenTime {
	/// From the index's milliseconds, when both ends are known.
	pub fn from_ms(start_ms: Option<i64>, end_ms: Option<i64>) -> Option<Self> {
		Some(Self {
			start_seconds: start_ms? as f64 / 1000.0,
			end_seconds: end_ms? as f64 / 1000.0,
		})
	}
}

/// One page of hits.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
pub struct ContentSearchOutput {
	pub hits: Vec<ContentSearchHit>,
	/// Pass back as `cursor` to read the next page. Absent on the last page.
	pub next_cursor: Option<String>,
	/// Files under the scope whose text is not indexed yet, or was read from
	/// bytes that have since changed. Zero when the index is current there.
	pub pending_files: u64,
}
