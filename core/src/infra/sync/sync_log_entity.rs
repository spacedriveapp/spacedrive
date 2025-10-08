//! Sync log entity
//!
//! The sync log is an append-only, sequentially-ordered log of all state changes
//! per library. It enables synchronization of changes across devices.

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::ActiveValue;
use serde::{Deserialize, Serialize};

/// Sync log entry model (SeaORM entity)
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "sync_log")]
pub struct Model {
	/// Internal database ID (auto-increment)
	#[sea_orm(primary_key)]
	pub id: i32,

	/// Monotonic sequence number (unique per library)
	/// This is the primary ordering field for sync
	#[sea_orm(unique)]
	pub sequence: i64,

	/// Device that created this entry
	pub device_id: Uuid,

	/// When this change was made
	pub timestamp: DateTimeUtc,

	/// Model type ("album", "tag", "entry", "bulk_operation")
	pub model_type: String,

	/// UUID of the changed record
	pub record_id: Uuid,

	/// Type of change ("insert", "update", "delete", "bulk_insert")
	pub change_type: String,

	/// Version number for optimistic concurrency control
	pub version: i64,

	/// JSON data payload containing the full model data
	#[sea_orm(column_type = "Text")]
	pub data: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

/// High-level sync log entry (for application use)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncLogEntry {
	pub sequence: u64,
	pub device_id: Uuid,
	pub timestamp: DateTime<Utc>,
	pub model_type: String,
	pub record_id: Uuid,
	pub change_type: ChangeType,
	pub version: i64,
	pub data: serde_json::Value,
}

impl SyncLogEntry {
	/// Convert from SeaORM model to application type
	pub fn from_model(model: Model) -> Result<Self, serde_json::Error> {
		Ok(Self {
			sequence: model.sequence as u64,
			device_id: model.device_id,
			timestamp: model.timestamp.into(),
			model_type: model.model_type,
			record_id: model.record_id,
			change_type: ChangeType::from_str(&model.change_type),
			version: model.version,
			data: serde_json::from_str(&model.data)?,
		})
	}

	/// Convert to SeaORM active model for insertion
	pub fn to_active_model(&self) -> ActiveModel {
		ActiveModel {
			id: ActiveValue::NotSet,
			sequence: ActiveValue::Set(self.sequence as i64),
			device_id: ActiveValue::Set(self.device_id),
			timestamp: ActiveValue::Set(self.timestamp.into()),
			model_type: ActiveValue::Set(self.model_type.clone()),
			record_id: ActiveValue::Set(self.record_id),
			change_type: ActiveValue::Set(self.change_type.to_string()),
			version: ActiveValue::Set(self.version),
			data: ActiveValue::Set(self.data.to_string()),
		}
	}
}

/// Type of change in sync log
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeType {
	Insert,
	Update,
	Delete,
	BulkInsert,
}

impl ChangeType {
	pub fn to_string(&self) -> String {
		match self {
			ChangeType::Insert => "insert".to_string(),
			ChangeType::Update => "update".to_string(),
			ChangeType::Delete => "delete".to_string(),
			ChangeType::BulkInsert => "bulk_insert".to_string(),
		}
	}

	pub fn from_str(s: &str) -> Self {
		match s {
			"insert" => ChangeType::Insert,
			"update" => ChangeType::Update,
			"delete" => ChangeType::Delete,
			"bulk_insert" => ChangeType::BulkInsert,
			_ => ChangeType::Insert, // Default fallback
		}
	}
}

/// Re-export the SeaORM model as SyncLogModel for clarity
pub type SyncLogModel = Model;
pub type SyncLogActiveModel = ActiveModel;
pub type SyncLogEntity = Entity;
