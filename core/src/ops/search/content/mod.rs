//! # Indexed content search
//!
//! `search.content` answers a word query from the content text index: the
//! text extracted from a source's documents and code, kept in `text.db`
//! beside the source store on the device that owns the source. Hits come back
//! ranked by BM25 with a snippet around the best match.
//!
//! It complements `search.grep`. Grep reads files as they are on disk, takes
//! a regular expression and needs no index, so its cost grows with the tree.
//! This answers from the index in milliseconds whatever the tree's size, but
//! matches words rather than patterns and knows only what the last pass read.
//! A file changed since then is left out of results until its new text lands,
//! and `pending_files` says how far behind the index is.
//!
//! `search.content.rebuild` drops a source's index and reads every file
//! again. The index is part of the store's generation, so nothing is lost.
//!
//! ## Example
//! ```rust,no_run
//! use sd_core::{domain::addressing::SdPath, ops::search::content::ContentSearchInput};
//!
//! let input = ContentSearchInput {
//! 	scope: SdPath::local("/home/me/notes"),
//! 	query: "\"quarterly report\" budget".into(),
//! 	kinds: vec![],
//! 	max_results: None,
//! 	cursor: None,
//! };
//! ```

#[cfg(any(feature = "video-shots", feature = "ai-providers"))]
pub mod descriptions;
pub mod input;
pub mod output;
pub mod query;
pub mod rebuild;

pub use input::*;
pub use output::*;
pub use query::*;
pub use rebuild::*;
