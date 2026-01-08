//! Platform-agnostic filesystem watcher
//!
//! `sd-fs-watcher` provides a clean, storage-agnostic interface for watching
//! filesystem changes. It handles platform-specific quirks (like macOS rename
//! detection) internally and emits normalized events.
//!
//! # Architecture
//!
//! The crate is organized into layers:
//!
//! - **FsWatcher**: Main interface for watching paths and receiving events
//! - **PlatformHandler**: Platform-specific event processing (rename detection, buffering)
//! - **FsEvent/FsEventKind**: Normalized, storage-agnostic event types
//!
//! # Key Features
//!
//! - **Storage Agnostic**: No knowledge of databases, libraries, or UUIDs
//! - **Rename Detection**: Handles macOS FSEvents rename quirks via inode tracking
//! - **Event Filtering**: Built-in filtering for temp files, hidden files, etc.
//! - **Reference Counting**: Multiple watchers on the same path share resources
//! - **Broadcast Events**: Multiple subscribers can receive events concurrently
//!
//! # Example
//!
//! ```ignore
//! use sd_fs_watcher::{FsWatcher, WatchConfig, WatcherConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create watcher with default config
//!     let watcher = FsWatcher::new(WatcherConfig::default());
//!     watcher.start().await?;
//!
//!     // Subscribe to events
//!     let mut rx = watcher.subscribe();
//!
//!     // Watch a directory recursively
//!     let _handle = watcher.watch("/path/to/watch", WatchConfig::recursive()).await?;
//!
//!     // Process events
//!     while let Ok(event) = rx.recv().await {
//!         match event.kind {
//!             sd_fs_watcher::FsEventKind::Create => println!("Created: {}", event.path.display()),
//!             sd_fs_watcher::FsEventKind::Modify => println!("Modified: {}", event.path.display()),
//!             sd_fs_watcher::FsEventKind::Remove => println!("Removed: {}", event.path.display()),
//!             sd_fs_watcher::FsEventKind::Rename { from, to } => {
//!                 println!("Renamed: {} -> {}", from.display(), to.display())
//!             }
//!         }
//!     }
//!
//!     Ok(())
//! }
//! ```

mod config;
mod error;
mod event;
mod platform;
mod watcher;

pub use config::{EventFilters, WatchConfig, WatcherConfig};
pub use error::{Result, WatcherError};
pub use event::{FsEvent, FsEventKind, RawEventKind, RawNotifyEvent};
pub use platform::{EventHandler, PlatformHandler};
pub use watcher::{FsWatcher, WatchHandle};

// Re-export notify types that users might need
pub use notify::RecursiveMode;

