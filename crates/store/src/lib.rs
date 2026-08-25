//! # sd-store — the source store
//!
//! Spacedrive is a set of sources. A source has an origin, an ingest and a
//! store, and a filesystem source differs from an adapter source only in
//! ingest. This crate is the store: one SQLite file per source, the same shape
//! whatever wrote it.
//!
//! ```text
//! record          one row per indexed thing, (type, external_id) unique
//! facet_<model>   the type's own columns, keyed by record_uuid
//! content         the identity of the bytes a record points at
//! edge            relationships between records in this source
//! record_overlay  what no ingest produced: a person's assertions
//! search_index    FTS5 over the fields the search contract names
//! ```
//!
//! One shape is the point. Cross-source search and the catalog both join on
//! it; two shapes would mean two of everything downstream. [`TrustTier`]
//! travels with a source rather than with its ingest for the same reason.
//!
//! `edge` relates records within one source. Nothing here spans sources —
//! that arrives with the catalog.
//!
//! **A store has two halves, and only one of them can ever be rebuilt.** The
//! generation (`record`, `facet_*`, `content`, `edge`, `search_index`) can be
//! rebuilt for as long as its origin still answers. `record_overlay` holds what
//! no ingest produced, so nothing rebuilds it on any day, for any source.
//!
//! Whether the origin still answers varies per source, varies over time, and
//! changes without an event: a detached drive, a revoked token, a closed
//! account. So no path here may assume a store can be rebuilt. Rebuild is an
//! operation a person asks for when the origin is known to be answering.
//! `docs/core/design/source-durability.md` carries the reasoning and what sync
//! needs the assertion tables to reserve.
//!
//! What is *not* here: no adapter runtime, no source registry, no cross-source
//! router, no job system. Those belong to whoever owns more than one source.

pub mod db;
pub mod error;
pub mod fts;
pub mod record;
pub mod schema;
pub mod source;

use serde::{Deserialize, Serialize};

pub use db::{ItemRow, SourceDb};
pub use error::{Error, Result};
pub use record::{ContentIdentity, Record};
pub use schema::{DataTypeSchema, FieldType, ModelDef};
pub use source::SourceManager;

/// How much a source's content is trusted. Adapter manifests declare it and it
/// is stored on the source row; screening policy keys on it once screening
/// exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TrustTier {
	/// User-created content (Obsidian notes, local files, personal calendar).
	Authored,
	/// Shared / multi-author spaces (Slack, Discord, GitHub).
	Collaborative,
	/// Third-party content (email inbox, RSS, web bookmarks, browser history).
	#[default]
	External,
}

impl TrustTier {
	/// Parse from a string, defaulting to `External` for unknown values.
	pub fn from_str_or_default(s: &str) -> Self {
		match s {
			"authored" => Self::Authored,
			"collaborative" => Self::Collaborative,
			"external" => Self::External,
			_ => {
				tracing::warn!(value = s, "unknown trust_tier, defaulting to 'external'");
				Self::External
			}
		}
	}

	/// Canonical string representation.
	pub fn as_str(&self) -> &'static str {
		match self {
			Self::Authored => "authored",
			Self::Collaborative => "collaborative",
			Self::External => "external",
		}
	}
}

impl std::fmt::Display for TrustTier {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(self.as_str())
	}
}
