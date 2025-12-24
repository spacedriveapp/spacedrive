//! Location service settings operations
//!
//! Provides query and actions for managing per-location service configuration
//! for watcher, stale detector, and sync services.

mod get;
mod trigger_stale;
mod update;

pub use get::*;
pub use trigger_stale::*;
pub use update::*;
