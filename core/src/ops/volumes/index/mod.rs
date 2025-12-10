//! Volume indexing operation module

pub mod action;
pub mod output;

use crate::ops::indexing::job::IndexScope;
use serde::{Deserialize, Serialize};
use specta::Type;

pub use action::IndexVolumeAction;
pub use output::IndexVolumeOutput;

/// Input for volume indexing action
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct IndexVolumeInput {
	/// Volume fingerprint to index
	pub fingerprint: String,
	/// Indexing scope (defaults to Recursive for full volume)
	#[serde(default = "default_scope")]
	pub scope: IndexScope,
}

fn default_scope() -> IndexScope {
	IndexScope::Recursive
}
