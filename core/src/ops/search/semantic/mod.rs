//! # Semantic search
//!
//! `search.semantic` finds files by what they mean, not only by the words
//! they use. It ranks the files under a path by two signals at once: BM25
//! over the content text index (`text.db`) and cosine similarity over the
//! vector index (`vectors.db`), fused by reciprocal rank. A file found by
//! both ranks above one found by either alone, and a file whose vectors the
//! background pass has not written yet is still found by its words.
//!
//! The vectors come from the text the index already holds: extracted
//! document text, transcripts, and scene and caption descriptions, cut into
//! chunks of about a thousand characters. Only the source's deep scope is
//! embedded, which bounds the cost to the folders a person keeps.
//!
//! Embedding needs a local text model. A device without one, or a build
//! without local inference, answers `search.semantic.status` with the
//! reason and keeps `search.content` for words.
//!
//! ## Example
//! ```rust,no_run
//! use sd_core::{domain::addressing::SdPath, ops::search::semantic::SemanticSearchInput};
//!
//! let input = SemanticSearchInput {
//! 	scope: SdPath::local("/home/me/Documents"),
//! 	query: "notes about the kitchen renovation budget".into(),
//! 	kinds: vec![],
//! 	max_results: None,
//! 	cursor: None,
//! };
//! ```

pub mod embedder;
pub mod input;
pub mod output;
pub mod query;
pub mod status;

pub use input::*;
pub use output::*;
pub use query::*;
pub use status::*;
