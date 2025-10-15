use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Represents a sync relationship between two directories
///
/// A SyncConduit defines a persistent sync configuration between a source and target directory,
/// tracking sync state, configuration, and statistics.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "sync_conduit")]
pub struct Model {
	#[sea_orm(primary_key)]
	pub id: i32,

	#[sea_orm(unique)]
	pub uuid: Uuid,

	/// Source directory entry ID (must be a directory)
	pub source_entry_id: i32,

	/// Target directory entry ID (must be a directory)
	pub target_entry_id: i32,

	/// Sync mode: "mirror", "bidirectional", or "selective"
	pub sync_mode: String,

	/// Whether this conduit is active
	pub enabled: bool,

	/// Sync schedule: "instant", "interval:5m", or "manual"
	pub schedule: String,

	/// Whether to use indexer rules for filtering
	pub use_index_rules: bool,

	/// Optional override for index mode (e.g., "shallow", "deep")
	pub index_mode_override: Option<String>,

	/// Number of parallel file transfers
	pub parallel_transfers: i32,

	/// Optional bandwidth limit in MB/s
	pub bandwidth_limit_mbps: Option<i32>,

	/// Timestamp of last successful sync completion
	pub last_sync_completed_at: Option<DateTime<Utc>>,

	/// Current sync generation number (increments on each sync)
	pub sync_generation: i64,

	/// Last error message if sync failed
	pub last_sync_error: Option<String>,

	/// Total number of syncs performed
	pub total_syncs: i64,

	/// Total number of files synced
	pub files_synced: i64,

	/// Total bytes transferred
	pub bytes_transferred: i64,

	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
	/// Foreign key to source entry
	#[sea_orm(
		belongs_to = "super::entry::Entity",
		from = "Column::SourceEntryId",
		to = "super::entry::Column::Id"
	)]
	SourceEntry,

	/// Foreign key to target entry
	#[sea_orm(
		belongs_to = "super::entry::Entity",
		from = "Column::TargetEntryId",
		to = "super::entry::Column::Id"
	)]
	TargetEntry,

	/// One-to-many relationship with sync generations
	#[sea_orm(has_many = "super::sync_generation::Entity")]
	SyncGenerations,
}

impl Related<super::sync_generation::Entity> for Entity {
	fn to() -> RelationDef {
		Relation::SyncGenerations.def()
	}
}

impl ActiveModelBehavior for ActiveModel {}

/// Sync mode variants
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncMode {
	/// One-way sync from source to target with automatic cleanup
	Mirror,
	/// Two-way sync with conflict detection
	Bidirectional,
	/// Intelligent local storage management (future)
	Selective,
}

impl SyncMode {
	pub fn as_str(&self) -> &'static str {
		match self {
			Self::Mirror => "mirror",
			Self::Bidirectional => "bidirectional",
			Self::Selective => "selective",
		}
	}

	pub fn from_str(s: &str) -> Option<Self> {
		match s {
			"mirror" => Some(Self::Mirror),
			"bidirectional" => Some(Self::Bidirectional),
			"selective" => Some(Self::Selective),
			_ => None,
		}
	}
}

impl std::fmt::Display for SyncMode {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.as_str())
	}
}
