//! File search operations

use serde::{Deserialize, Serialize};
use specta::Type;

pub mod ephemeral_search;
pub mod input;
pub mod output;
pub mod pipeline;
pub mod query;
pub mod store_search;
pub mod tag_scope;

#[cfg(test)]
mod tests;

pub use input::*;
pub use output::*;
pub use query::*;

/// Indicates which filters are available for a given search type
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, Hash, PartialEq, Eq)]
pub enum FilterKind {
	FileTypes,
	DateRange,
	SizeRange,
	ContentTypes,
	Tags,
	Hidden,
	Archived,     // Not implemented yet
	AtRisk,       // Redundancy: content on exactly one volume
	OnVolumes,    // Redundancy: content present on specific volumes
	NotOnVolumes, // Redundancy: content absent from specific volumes
	VolumeCount,  // Redundancy: min/max volume count
}
