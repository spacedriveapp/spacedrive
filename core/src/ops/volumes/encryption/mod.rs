//! Volume encryption query module
//!
//! Provides queries for checking encryption status of volumes and paths.
//! Used by the frontend to determine optimal secure delete strategies.

pub mod output;
pub mod query;

pub use output::{PathEncryptionInfo, VolumeEncryptionOutput};
pub use query::{VolumeEncryptionQuery, VolumeEncryptionQueryInput};
