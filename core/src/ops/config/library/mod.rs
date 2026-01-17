//! Library configuration operations
//!
//! Operations for reading and updating per-library configuration.

pub mod get;
pub mod update;

pub use get::{GetLibraryConfigQuery, GetLibraryConfigQueryInput};
pub use update::{UpdateLibraryConfigAction, UpdateLibraryConfigInput, UpdateLibraryConfigOutput};
