//! High-performance ephemeral index storage backend
//!
//! This module provides memory-efficient storage for ephemeral file indexes,
//! achieving 3-4x memory reduction compared to HashMap<PathBuf, EntryMetadata>.
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
//!
//! ## Memory Comparison
//!
//! | Files | HashMap Approach | This Module | Reduction |
//! |-------|------------------|-------------|-----------|
//! | 10K   | 2-3 MB           | 0.5 MB      | 4-6x      |
//! | 100K  | 20-30 MB         | 5 MB        | 4-6x      |
//! | 1M    | 200-300 MB       | 50 MB       | 4-6x      |
//!
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
pub mod index_cache;
pub mod registry;
pub mod types;

// Re-export public types
pub use arena::NodeArena;
pub use cache::NameCache;
pub use index_cache::EphemeralIndexCache;
pub use registry::NameRegistry;
pub use types::{EntryId, FileNode, FileType, MaybeEntryId, NameRef, NodeState, PackedMetadata};
