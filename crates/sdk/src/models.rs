//! Extension Data Models - Person, Album, Place, etc.
//!
//! This is the core concept: Extensions define custom data structures
//! that are stored in the VDFS and participate in tags, collections, sync.
//!
//! NOT to be confused with AI models (machine learning) - those are in ai.rs

use crate::types::*;
use serde::{de::DeserializeOwned, Serialize};

/// Marker trait for extension-defined data models
///
/// Models are stored in the `models` database table and can be:
/// - Content-scoped: Attached to a ContentIdentity (PhotoAnalysis)
/// - Standalone: Independent entities (Person, Album, Place)
/// - Entry-scoped: Tied to a specific path (rare)
pub trait ExtensionModel: Serialize + DeserializeOwned + Send + Sync {
	/// Model type name (e.g., "Person", "Album")
	const MODEL_TYPE: &'static str;

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
