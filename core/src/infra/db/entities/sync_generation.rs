use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// Tracks individual sync operations for history and verification
///
/// Each sync execution creates a new generation with metrics and verification status.
/// Generations enable sync history tracking and verification of sync consistency.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "sync_generation")]
pub struct Model {
	#[sea_orm(primary_key)]
	pub id: i32,

	/// Foreign key to sync_conduit
	pub conduit_id: i32,

	/// Generation number (monotonically increasing)
	pub generation: i64,

	/// When this sync generation started
	pub started_at: DateTime<Utc>,

	/// When this sync generation completed (None if still running)
	pub completed_at: Option<DateTime<Utc>>,

	/// Number of files copied during this sync
	pub files_copied: i32,

	/// Number of files deleted during this sync
	pub files_deleted: i32,

	/// Number of conflicts resolved during this sync
	pub conflicts_resolved: i32,

	/// Total bytes transferred during this sync
	pub bytes_transferred: i64,

	/// Number of errors encountered during this sync
	pub errors_encountered: i32,

	/// When verification was performed (None if not yet verified)
	pub verified_at: Option<DateTime<Utc>>,

	/// Verification status: "unverified", "waiting_watcher", "waiting_library_sync", "verified", "failed:<reason>"
	pub verification_status: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
	/// Foreign key to sync_conduit
	#[sea_orm(
		belongs_to = "super::sync_conduit::Entity",
		from = "Column::ConduitId",
		to = "super::sync_conduit::Column::Id"
	)]
	SyncConduit,
}

impl Related<super::sync_conduit::Entity> for Entity {
	fn to() -> RelationDef {
		Relation::SyncConduit.def()
	}
}

impl ActiveModelBehavior for ActiveModel {}

/// Verification status variants
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerificationStatus {
	/// Sync completed, not yet verified
	Unverified,
	/// Waiting for filesystem watcher to update index
	WaitingWatcher,
	/// Waiting for library sync to propagate changes
	WaitingLibrarySync,
	/// Verification query confirmed consistency
	Verified,
}

impl VerificationStatus {
	pub fn as_str(&self) -> &'static str {
		match self {
			Self::Unverified => "unverified",
			Self::WaitingWatcher => "waiting_watcher",
			Self::WaitingLibrarySync => "waiting_library_sync",
			Self::Verified => "verified",
		}
	}

	pub fn from_str(s: &str) -> Option<Self> {
		match s {
			"unverified" => Some(Self::Unverified),
			"waiting_watcher" => Some(Self::WaitingWatcher),
			"waiting_library_sync" => Some(Self::WaitingLibrarySync),
			"verified" => Some(Self::Verified),
			_ if s.starts_with("failed:") => None, // Failed states are dynamic
			_ => None,
		}
	}

	pub fn failed(reason: &str) -> String {
		format!("failed:{}", reason)
	}
}

impl std::fmt::Display for VerificationStatus {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.as_str())
	}
}
