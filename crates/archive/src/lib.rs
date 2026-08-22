//! # sd-archive — adapter-backed sources
//!
//! Spacedrive is a set of sources. A source has an origin, an ingest and a
//! store; a filesystem source and an adapter source differ only in ingest.
//! This crate carries the store, and one ingest path: adapters, for origins
//! that are not a filesystem — mail, notes, messages, bookmarks, calendars,
//! contacts.
//!
//! ## Core capabilities:
//!
//! - **Universal indexing** — Adapters ingest data from external sources via a
//!   script-based protocol (stdin/stdout JSONL).
//!
//! - **Full-text search** — SQLite FTS5 per source, routed across sources so
//!   results from different data types come back in one shape.
//!
//! - **Schema-driven sources** — Each data source has its own SQLite index and
//!   TOML schema. Sources are portable.
//!
//! ## Two files
//!
//! A source **store** is one SQLite file per source. Every row in it sits in
//! the universal record table ([`record`]), with type-specific columns in facet
//! tables generated from the data type's TOML models. It is user data, not a
//! cache: a re-scan is a recovery path, not something the design assumes it can
//! fall back on. Plenty of origins cannot be re-scanned on demand — a detached
//! drive, a revoked token, a closed account.
//!
//! The **library** ([`library`]) holds what no ingest produced: overlays,
//! curated groupings and cross-source edges. It keys on
//! `(source_id, type, external_id)` rather than the record uuid, so assertions
//! rebind when a source is removed and added back.
//!
//! Splitting them across two files is the shape this crate has today, not the
//! one it is heading for. `docs/plans/2026-08-22-source-convergence.md` P1
//! settles where the durable tables live.
//!
//! ## Architecture
//!
//! This crate is designed to be embedded in Spacedrive's core. It does not include
//! the job system or operation layer — those live in `core/src/ops/sources/`.
//!
//! ```text
//! Core
//!   -> Library
//!     -> SourceManager (wraps sd-archive Engine)
//!       -> Engine
//!         -> AdapterRegistry
//!         -> Registry + Library   (registry.db)
//!         -> SourceDb             (per source: records + facets)
//!         -> SearchRouter
//! ```

pub mod adapter;
pub mod db;
pub mod engine;
pub mod error;
pub mod library;
pub mod record;
pub mod registry;
pub mod schema;
pub mod search;
pub mod source;

// Re-export primary types at crate root
pub use adapter::script::ConfigField;
pub use adapter::{AdapterInfo, AdapterUpdateResult, SyncReport};
pub use engine::{Engine, EngineConfig};
pub use error::{Error, Result};
pub use library::{Grouping, LibEdge, Library, RecordKey};
pub use record::{ContentIdentity, Record};
pub use registry::{DataTypeInfo, NewSource, Registry, SourceInfo, TrustTier};
pub use schema::{DataTypeSchema, FieldType, ModelDef};
pub use search::{SearchFilter, SearchResult};
