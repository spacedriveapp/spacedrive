//! # sd-archive — adapter-backed sources
//!
//! Spacedrive is a set of sources. A source has an origin, an ingest and a
//! store; a filesystem source and an adapter source differ only in ingest.
//! The store is [`sd_store`]. This crate carries one ingest — adapters, for
//! origins that are not a filesystem: mail, notes, messages, bookmarks,
//! calendars, contacts — plus the registry of which sources exist and the
//! router that searches across them.
//!
//! ## Core capabilities:
//!
//! - **Adapter ingest** — external processes speaking a JSONL protocol over
//!   stdin/stdout, declared by an `adapter.toml` manifest.
//!
//! - **Full-text search** — SQLite FTS5 per source, routed across sources so
//!   results from different data types come back in one shape.
//!
//! - **Schema-driven sources** — each data type declares its models in TOML,
//!   and the store generates the facet tables and search index from them.
//!
//! ## Architecture
//!
//! ```text
//! Core
//!   -> library.db `sources`       (which sources exist, whatever fills them)
//!   -> SourceManager (wraps Engine)
//!     -> Engine
//!       -> AdapterRegistry
//!       -> sd_store::SourceDb     (per source: records, facets, overlays)
//!       -> SearchRouter
//! ```
//!
//! What is left here is the adapter runtime and the stores it writes. Which
//! sources exist is a question for the library, so the engine is handed a
//! [`SourceRef`] rather than looking one up.

pub mod adapter;
pub mod engine;
pub mod error;
pub mod search;

pub use adapter::script::ConfigField;
pub use adapter::{AdapterInfo, AdapterUpdateResult, SyncReport};
pub use engine::{AdapterFacts, Engine, EngineConfig, SourceRef};
pub use error::{Error, Result};
pub use search::{SearchFilter, SearchResult};

// The store types callers need in the same breath as the engine.
pub use sd_store::{db, record, schema, DataTypeSchema, SourceDb, TrustTier};
