//! Filesystem Watcher Service
//!
//! Wraps `sd-fs-watcher` for platform-agnostic filesystem event detection.
//!
//! ## Architecture
//!
//! - **FsWatcherService**: Detects filesystem changes, emits events via broadcast channel
//! - **Handlers** (in `ops/indexing/handlers/`): Subscribe to events and route them
//! - **WatcherStateTracker**: Tracks watcher lifecycle for stale detection decisions
//!
//! The old monolithic `LocationWatcher` is preserved in `watcher_old/` for reference.

mod service;
mod state_tracker;

pub use crate::ops::indexing::handlers::LocationMeta;
pub use service::{FsWatcherService, FsWatcherServiceConfig};
pub use state_tracker::WatcherStateTracker;
