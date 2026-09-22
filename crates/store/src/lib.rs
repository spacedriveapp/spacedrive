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
//! facet_file      the filesystem facet, one model like any other
//! content         the identity of the bytes a record points at
//! edge            relationships between records in this source
//! record_overlay  what no ingest produced: a person's assertions
//! tag_definition  the tags this source uses, named, colored, hierarchical
//! tag_assertion   which tags apply to which records and content
//! search_index    FTS5 over the fields the search contract names
//! ```
//!
//! One shape is the point. Every question across sources reads each store and
//! joins on it; two shapes would mean two of everything downstream.
//! [`TrustTier`] travels with a source rather than with its ingest for the same
//! reason.
//!
//! `edge` relates records within one source. Nothing here spans sources: a
//! question across them asks each store and merges the answers.
//!
//! **A store has two halves, and only one of them can ever be rebuilt.** The
//! generation (`record`, `facet_*`, `content`, `edge`, `search_index`) can be
//! rebuilt for as long as its origin still answers. `record_overlay` and the
//! tag tables hold what no ingest produced, so nothing rebuilds them on any
//! day, for any source.
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

pub mod content;
pub mod db;
pub mod error;
pub mod file;
pub mod fts;
pub mod read;
pub mod record;
pub mod revision;
pub mod schema;
pub mod source;
pub mod tags;

use serde::{Deserialize, Serialize};

pub use content::{uuid_for, ContentId, CONTENT_NAMESPACE};
pub use db::{ItemRow, OverlayEvidence, SourceDb, Stamp};
pub use error::{Error, Result};
pub use file::{
	content_of, copies_of_content, count_files_needing_content, count_files_needing_verification,
	duplicate_copies, files_needing_content, files_needing_verification, filesystem_schema,
	mark_content_unreadable, ContentCopy, FileKind, FileWrite, Ledger, Observation, PendingContent,
	PendingVerification, Resolution, SubtreeRename, Watermark,
};
pub use read::{FsEntry, TitleMatches};
pub use record::{ContentIdentity, Record, RECORD_SCHEMA};
pub use revision::Revision;
pub use schema::{DataTypeSchema, FieldType, ModelDef};
pub use source::SourceManager;
pub use tags::{
	normalize_tag_path, slug_for_path, AppliedTag, TagAssertion, TagDefinition, TAG_NAMESPACE,
};

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
