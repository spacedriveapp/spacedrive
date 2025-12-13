//! Library information output types
//!
//! Note: The canonical resource type for libraries is `crate::domain::Library`.
//! This module re-exports it for backwards compatibility.

// Re-export the domain Library type as LibraryInfoOutput for backwards compatibility
pub use crate::domain::Library as LibraryInfoOutput;
