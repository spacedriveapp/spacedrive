//! Audit log entity for tracking user actions

use sea_orm::entity::prelude::*;
use sea_orm::Set;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "audit_log")]
pub struct Model {
	#[sea_orm(primary_key)]
	pub id: i32,

	#[sea_orm(unique)]
	pub uuid: String,

	#[sea_orm(indexed)]
	pub action_type: String,

	#[sea_orm(indexed)]
	pub actor_device_id: String,

	pub targets: String,

	#[sea_orm(indexed)]
	pub status: ActionStatus,

	#[sea_orm(indexed, nullable)]
	pub job_id: Option<String>,

	pub created_at: DateTimeUtc,
	pub completed_at: Option<DateTimeUtc>,

	pub error_message: Option<String>,

	pub result_payload: Option<String>,

	pub version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum ActionStatus {
	#[sea_orm(string_value = "in_progress")]
	InProgress,
	#[sea_orm(string_value = "completed")]
	Completed,
	#[sea_orm(string_value = "failed")]
	Failed,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {
	fn new() -> Self {
		Self {
			uuid: Set(Uuid::new_v4().to_string()),
			created_at: Set(chrono::Utc::now()),
			version: Set(1),
			..ActiveModelTrait::default()
		}
	}
}

// Syncable Implementation
//
// Audit logs are SHARED resources using HLC-ordered log-based replication.
// This creates a unified audit trail across all devices with causal ordering.
// All devices can see what actions were performed by any device in the library.
impl crate::infra::sync::Syncable for Model {
	const SYNC_MODEL: &'static str = "audit_log";

	fn sync_id(&self) -> Uuid {
		Uuid::parse_str(&self.uuid).expect("Invalid UUID in audit_log")
	}

	fn version(&self) -> i64 {
		self.version
	}

	fn exclude_fields() -> Option<&'static [&'static str]> {
		// Don't sync database IDs, timestamps, or internal job tracking
		Some(&["id", "created_at", "updated_at", "job_id"])
	}

	fn sync_depends_on() -> &'static [&'static str] {
		&[] // No FK dependencies
	}

	fn foreign_key_mappings() -> Vec<crate::infra::sync::FKMapping> {
		vec![] // actor_device_id is already a UUID string
	}

	/// Query audit logs for backfill (shared resources)
	///
	/// Returns ALL audit logs across all devices for unified audit trail
	async fn query_for_sync(
		_device_id: Option<Uuid>,
		since: Option<chrono::DateTime<chrono::Utc>>,
		_cursor: Option<(chrono::DateTime<chrono::Utc>, Uuid)>,
		batch_size: usize,
		db: &DatabaseConnection,
	) -> Result<Vec<(Uuid, serde_json::Value, chrono::DateTime<chrono::Utc>)>, sea_orm::DbErr> {
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};

		let mut query = Entity::find();

		// Filter by timestamp if specified (for incremental sync)
		if let Some(since_time) = since {
			query = query.filter(Column::CreatedAt.gte(since_time));
		}

		// Apply batch limit
		query = query.limit(batch_size as u64);

		let results = query.all(db).await?;

		// Convert to sync format
		let mut sync_results = Vec::new();
		for log in results {
			let json = match log.to_sync_json() {
				Ok(j) => j,
				Err(e) => {
					tracing::warn!(error = %e, uuid = %log.uuid, "Failed to serialize audit_log for sync");
					continue;
				}
			};

			sync_results.push((log.sync_id(), json, log.created_at));
		}

		Ok(sync_results)
	}

	/// Apply shared change with HLC-ordered conflict resolution
	async fn apply_shared_change(
		entry: crate::infra::sync::SharedChangeEntry,
		db: &DatabaseConnection,
	) -> Result<(), sea_orm::DbErr> {
		use crate::infra::sync::ChangeType;

		match entry.change_type {
			ChangeType::Insert | ChangeType::Update => {
				let data = entry.data.as_object().ok_or_else(|| {
					sea_orm::DbErr::Custom("Audit log data is not an object".to_string())
				})?;

				let uuid: String = serde_json::from_value(
					data.get("uuid")
						.ok_or_else(|| sea_orm::DbErr::Custom("Missing uuid".to_string()))?
						.clone(),
				)
				.map_err(|e| sea_orm::DbErr::Custom(format!("Invalid uuid: {}", e)))?;

				use sea_orm::{ActiveValue::NotSet, Set};

				let active = ActiveModel {
					id: NotSet,
					uuid: Set(uuid),
					action_type: Set(serde_json::from_value(
						data.get("action_type")
							.cloned()
							.unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					actor_device_id: Set(serde_json::from_value(
						data.get("actor_device_id")
							.cloned()
							.unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					targets: Set(serde_json::from_value(
						data.get("targets")
							.cloned()
							.unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					status: Set(serde_json::from_value(
						data.get("status")
							.cloned()
							.unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					job_id: Set(None), // Excluded from sync (local-only)
					created_at: Set(chrono::Utc::now().into()),
					completed_at: Set(serde_json::from_value(
						data.get("completed_at")
							.cloned()
							.unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					error_message: Set(serde_json::from_value(
						data.get("error_message")
							.cloned()
							.unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					result_payload: Set(serde_json::from_value(
						data.get("result_payload")
							.cloned()
							.unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					version: Set(serde_json::from_value(
						data.get("version")
							.cloned()
							.unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
				};

				// Idempotent upsert by UUID
				Entity::insert(active)
					.on_conflict(
						sea_orm::sea_query::OnConflict::column(Column::Uuid)
							.update_columns([
								Column::ActionType,
								Column::ActorDeviceId,
								Column::Targets,
								Column::Status,
								Column::CompletedAt,
								Column::ErrorMessage,
								Column::ResultPayload,
								Column::Version,
							])
							.to_owned(),
					)
					.exec(db)
					.await?;
			}

			ChangeType::Delete => {
				// Delete by UUID (rare for audit logs, but supported)
				Entity::delete_many()
					.filter(Column::Uuid.eq(entry.record_uuid.to_string()))
					.exec(db)
					.await?;
			}
		}

		Ok(())
	}
}

// Register with sync system via inventory
crate::register_syncable_shared!(Model, "audit_log", "audit_log");
