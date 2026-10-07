//! Extension Data Models - Person, Album, Place, etc.
//!
//! This is the core concept: Extensions define custom data structures
//! that are stored in the VDFS and participate in tags, collections, sync.
//!
//! NOT to be confused with AI models (machine learning) - those are in ai.rs

use crate::types::*;
use serde::{de::DeserializeOwned, Serialize};

/// An extension-defined data model, implemented by `#[model]`.
///
/// A model is a record in the extension's own store with a facet table of
/// the struct's fields. It is either standalone, addressed by its own uuid
/// (Person, Album, Place), or content-scoped, addressed by the content it
/// describes (an analysis of a photo). `plugin_init` declares every model
/// listed in `#[extension(models = [...])]` to the host, which creates or
/// widens the facet table before any job runs.
pub trait ExtensionModel: Serialize + DeserializeOwned + Send + Sync {
	/// Model type name (e.g., "Person", "Album")
	const MODEL_TYPE: &'static str;

	/// The facet definition the host registers: the fields and their column
	/// types, as JSON. Derived from the struct by `#[model]`.
	const DEFINITION: &'static str;

	/// Get the model's UUID
	fn uuid(&self) -> Uuid;

	/// Generate search text for FTS5 indexing
	fn search_text(&self) -> String {
		String::new() // Default: no search text
	}
}

// Re-export for convenience
pub use crate::vdfs::{
	ModelQuery,
	VdfsContext, // Has model operations
};
