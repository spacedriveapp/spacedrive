//! # sd-archive — Spacedrive's Data Archival System
//!
//! A standalone crate for indexing external data sources beyond the filesystem.
//! Handles emails, notes, messages, bookmarks, calendar events, contacts, and more.
//!
//! ## Core capabilities:
//!
//! - **Universal indexing** — Adapters ingest data from external sources via a
//!   script-based protocol (stdin/stdout JSONL).
//!
//! - **Hybrid search** — Combines full-text search (SQLite FTS5) with semantic
//!   vector search (LanceDB + FastEmbed) merged via Reciprocal Rank Fusion.
//!
//! - **Safety screening** — Prompt Guard 2 classifies indexed text for injection
//!   attacks before it enters the search index.
//!
//! - **Schema-driven sources** — Each data source has its own SQLite database,
//!   vector index, and TOML schema. Sources are portable.
//!
//! ## Architecture
//!
//! This crate is designed to be embedded in Spacedrive's core. It does not include
//! the job system or operation layer — those live in `core/src/ops/sources/`.
//!
//! ```
//! Core
//!   -> Library
//!     -> SourceManager (wraps sd-archive Engine)
//!       -> Engine
//!         -> AdapterRegistry
//!         -> SourceDb
//!         -> SearchRouter
//!         -> EmbeddingModel
//! ```

pub mod adapter;
pub mod db;
pub mod embed;
pub mod engine;
pub mod error;
pub mod registry;
pub mod safety;
pub mod schema;
pub mod search;
pub mod source;

// Re-export primary types at crate root
pub use adapter::script::ConfigField;
pub use adapter::{AdapterInfo, AdapterUpdateResult, SyncReport};
pub use engine::{Engine, EngineConfig};
pub use error::{Error, Result};
pub use registry::{DataTypeInfo, NewSource, Registry, SourceInfo};
pub use safety::{SafetyMode, SafetyPolicy, SafetyVerdict, TrustTier};
pub use schema::{DataTypeSchema, FieldType, ModelDef};
pub use search::{SearchFilter, SearchResult};
