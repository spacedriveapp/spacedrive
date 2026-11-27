//! Output of `search.semantic` and `search.semantic.status`.

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::ops::{
	indexing::text_extract::TextKind,
	search::content::{SnippetFragment, SpokenTime},
};

/// One ranked file.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SemanticSearchHit {
	/// The file, relative to the scope, with `/` separators.
	pub path: String,
	/// What the matching text came from.
	pub kind: TextKind,
	/// The fused score. Larger is better; only the order within one
	/// response means anything.
	pub score: f64,
	/// Cosine similarity of the file's best chunk to the query, when the
	/// file has vectors.
	pub similarity: Option<f32>,
	/// Whether any of the query's words appear in the file.
	pub word_match: bool,
	/// Words around the best word match, with the matches marked, or the
	/// start of the closest chunk when no word matched.
	pub snippet: Vec<SnippetFragment>,
	/// The moment in a recording the hit is about.
	pub time: Option<SpokenTime>,
}

/// One page of hits.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
pub struct SemanticSearchOutput {
	pub hits: Vec<SemanticSearchHit>,
	/// Pass back as `cursor` to read the next page.
	pub next_cursor: Option<String>,
	/// Documents in the source's deep scope still waiting for vectors.
	/// Those are found by their words only until the pass reaches them.
	pub pending_documents: u64,
}

/// Whether semantic search runs for a source, and how far its index is.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
pub struct SemanticStatusOutput {
	pub available: bool,
	/// Why it is unavailable, when it is.
	pub reason: Option<String>,
	/// The embedder the source's vectors came from.
	pub model: Option<String>,
	/// Documents with vectors.
	pub documents: u64,
	pub chunks: u64,
	/// Documents in the deep scope still waiting for vectors.
	pub pending_documents: u64,
	/// The vector index's size on disk, in bytes.
	pub size_bytes: u64,
}
