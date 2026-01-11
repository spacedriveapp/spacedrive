//! File search operations

use serde::{Deserialize, Serialize};
use specta::Type;

pub mod ephemeral_search;
pub mod facets;
pub mod filters;
pub mod input;
pub mod output;
pub mod query;
pub mod sorting;

#[cfg(test)]
mod tests;

pub use facets::*;
pub use filters::*;
pub use input::*;
pub use output::*;
pub use query::*;
pub use sorting::*;

/// Indicates which index type was used for a search query
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, PartialEq, Eq)]
pub enum IndexType {
	/// Database FTS5 search (persistent index)
	Persistent,
	/// In-memory ephemeral search
	Ephemeral,
	/// Mix of both (future: hybrid searches)
	Hybrid,
}

/// Indicates which filters are available for a given search type
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, Hash, PartialEq, Eq)]
pub enum FilterKind {
	FileTypes,
	DateRange,
	SizeRange,
	ContentTypes,
	Tags,      // Persistent only
	Locations, // Persistent only
	Hidden,    // Not implemented yet
	Archived,  // Not implemented yet
}
