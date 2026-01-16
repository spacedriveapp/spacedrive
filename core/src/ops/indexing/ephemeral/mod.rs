//! High-performance ephemeral index storage backend
//!
//! This module provides memory-efficient storage for ephemeral file indexes,
//! achieving 3-4x memory reduction compared to HashMap<PathBuf, EntryMetadata>.
//!
//! The design of this system is heavily inspired by Cardinal's search-cache implementation,
//! particularly the memory-mapped arena storage, string interning, and snapshot persistence
//! patterns. See: https://github.com/cardisoft/cardinal
//!
//! ## Architecture
//!
//! ```text
//! EphemeralIndex
//! ├── NodeArena: Vec<FileNode>        - Contiguous node storage
//! ├── NameCache: BTreeSet<Box<str>>   - String interning pool
//! ├── NameRegistry: BTreeMap          - Fast name lookups
//! └── path_index: HashMap<PathBuf, EntryId>  - Path to node mapping
//! ```
//! ## Usage
//!
//! ```rust,ignore
//! use sd_core::ops::indexing::ephemeral::EphemeralIndex;
//!
//! // Create a unified index (supports multiple directory trees)
//! let mut index = EphemeralIndex::new();
//!
//! // Add entries with full paths - parent chain is created automatically
//! index.add_entry(path, uuid, metadata);
//!
//! // Query
//! let entry = index.get_entry(&path);
//! let children = index.list_directory(&parent);
//! ```

pub mod arena;
pub mod cache;
pub mod index;
pub mod name;
pub mod registry;
pub mod responder;
pub mod snapshot;
pub mod types;
pub mod writer;

// Re-export public types
pub use arena::NodeArena;
pub use cache::EphemeralIndexCache;
pub use index::{EphemeralIndex, EphemeralIndexStats};
pub use name::NameCache;
pub use registry::NameRegistry;
pub use snapshot::{get_snapshot_cache_dir, snapshot_path_for};
pub use types::{EntryId, FileNode, FileType, MaybeEntryId, NameRef, NodeState, PackedMetadata};
pub use writer::MemoryAdapter;
