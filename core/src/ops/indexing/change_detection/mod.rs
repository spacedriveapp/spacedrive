//! Change detection and handling for the indexing system.
//!
//! This module provides two complementary subsystems:
//!
//! 1. **Detection** (`detector.rs`): Batch scanning during indexer jobs.
//!    Compares database state against filesystem to identify changes.
//!
//! 2. **Handling** (`handler.rs`): Real-time response to watcher events.
//!    Applies changes (create/modify/move/delete) to storage.
//!
//! Both systems use the same `Change` type and share concepts like
//! inode-based move detection, ensuring consistent behavior.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                     Change Detection                        │
//! ├─────────────────────────────────────────────────────────────┤
//! │                                                             │
//! │  ┌─────────────┐            ┌─────────────┐                │
//! │  │  Detector   │            │   Handler   │                │
//! │  │  (batch)    │            │  (real-time)│                │
//! │  └──────┬──────┘            └──────┬──────┘                │
//! │         │                          │                        │
//! │         │     ┌─────────┐          │                        │
//! │         └────►│ Change  │◄─────────┘                        │
//! │               │  enum   │                                   │
//! │               └────┬────┘                                   │
//! │                    │                                        │
//! │         ┌──────────┴──────────┐                            │
//! │         ▼                     ▼                            │
//! │  ┌─────────────┐       ┌─────────────┐                     │
//! │  │ Persistent  │       │  Ephemeral  │                     │
//! │  │  Handler    │       │   Handler   │                     │
//! │  │ (database)  │       │ (in-memory) │                     │
//! │  └─────────────┘       └─────────────┘                     │
//! │                                                             │
//! └─────────────────────────────────────────────────────────────┘
//! ```

pub mod detector;
pub mod ephemeral;
pub mod handler;
pub mod persistent;
pub mod types;

// Re-export primary types
pub use detector::ChangeDetector;
pub use ephemeral::EphemeralChangeHandler;
pub use handler::{
	apply_batch, build_dir_entry, handle_create, handle_modify, handle_remove, handle_rename,
	path_exists_safe, should_filter_path, ChangeHandler,
};
pub use persistent::PersistentChangeHandler;
pub use types::{Change, ChangeConfig, ChangeMetadata, ChangeType, EntryRef};
