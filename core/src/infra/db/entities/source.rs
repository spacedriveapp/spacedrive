//! Source entity: the registry row for anything Spacedrive indexes.
//!
//! One row per source whatever its ingest. `data_type` is what forks them,
//! matching `_schema.data_type_id` in the source's own store: `filesystem` for
//! a walk, the adapter's data type otherwise.
//!
//! This row is the registration; the source's store, snapshot and thumbnail
//! cache are machine-local artifacts under `SourceDirs`. The registration is
//! library metadata and travels with the library, the artifacts are rebuilt
//! wherever they are needed.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "sources")]
pub struct Model {
	#[sea_orm(primary_key)]
	pub id: i32,
	/// Stable identity. Keys the source's directory under `SourceDirs`.
	#[sea_orm(unique)]
	pub uuid: Uuid,
	pub name: String,
	/// `filesystem`, or the adapter's data type.
	pub data_type: String,
	/// Null for a filesystem source: a walk has no adapter.
	pub adapter_id: Option<String>,
	pub config: String,

	/// Filesystem root at last attach. Null for an adapter source.
	pub root: Option<String>,
	/// The medium this source sits on, when it sits on one Spacedrive tracks.
	///
	/// Null is ordinary rather than exceptional: nested roots register as
	/// distinct sources, and adapter, cloud and fingerprint-less network
	/// sources have no volume at all. Where it is set, attachment is the
	/// volume's business, since a drive being present is a fact about the
	/// drive.
	pub volume_uuid: Option<Uuid>,

	/// What the store holds, as opposed to what the medium can hold.
	pub record_count: Option<i64>,
	pub directory_count: Option<i64>,
	pub total_bytes: Option<i64>,
	/// Deduplicated by content identity, so it is a claim about bytes rather
	/// than about records.
	pub unique_bytes: Option<i64>,
	pub last_indexed_at: Option<DateTimeUtc>,
	pub status: String,

	/// Travels with the source rather than with its ingest.
	pub trust_tier: String,
	pub created_at: DateTimeUtc,
	pub last_seen_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
	#[sea_orm(
		belongs_to = "super::volume::Entity",
		from = "Column::VolumeUuid",
		to = "super::volume::Column::Uuid"
	)]
	Volume,
}

impl Related<super::volume::Entity> for Entity {
	fn to() -> RelationDef {
		Relation::Volume.def()
	}
}

impl ActiveModelBehavior for ActiveModel {}

/// The data type a filesystem source carries. Declared in `sd_store`, repeated
/// here because the registry row is written before any store is opened.
pub const FILESYSTEM_DATA_TYPE: &str = "filesystem";
