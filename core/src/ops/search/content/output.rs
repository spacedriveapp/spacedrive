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
