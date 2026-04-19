//! Persistence for per-volume cloud sync state.
//!
//! Split from the change-detection types module so the indexer can depend on
//! the repository abstraction without pulling in the HTTP client used by
//! concrete detectors. The domain-level [`CloudSyncState`] struct is a
//! deliberate duplicate of the SeaORM [`Model`](super::super::super::super::infra::db::entities::cloud_sync_state::Model)
//! row — keeping the two separate prevents SeaORM's `DateTimeUtc` type
//! aliases from leaking into the rest of the codebase.

use crate::infra::db::entities::cloud_sync_state;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::{
	ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
};
use thiserror::Error;

/// Domain representation of a single `cloud_sync_state` row.
///
/// Uses `chrono::DateTime<Utc>` directly rather than SeaORM's `DateTimeUtc`
/// type alias so callers outside the repository do not depend on SeaORM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudSyncState {
	pub volume_id: i32,
	/// Provider id, matches `ChangeDetector::provider_id` (e.g. `"onedrive"`).
	pub provider: String,
	/// Latest baseline token. `None` until the first successful pass.
	pub change_token: Option<String>,
	/// When the last full re-listing completed.
	pub last_full_sync_at: Option<DateTime<Utc>>,
	pub last_incremental_at: Option<DateTime<Utc>>,
	/// Back-to-back failure count; reset on first success. Drives the
	/// scheduler's retry delay.
	pub consecutive_failures: i32,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
}

impl CloudSyncState {
	/// Seed a fresh row for a volume that has never been sync-scanned.
	///
	/// Timestamps are set to `now()` so SeaORM's `NotSet` does not resurface
	/// defaults at insert time; callers should normally go through
	/// [`CloudSyncStateRepository::upsert`] which takes care of this.
	pub fn new(volume_id: i32, provider: impl Into<String>) -> Self {
		let now = Utc::now();
		Self {
			volume_id,
			provider: provider.into(),
			change_token: None,
			last_full_sync_at: None,
			last_incremental_at: None,
			consecutive_failures: 0,
			created_at: now,
			updated_at: now,
		}
	}
}

impl From<cloud_sync_state::Model> for CloudSyncState {
	fn from(m: cloud_sync_state::Model) -> Self {
		Self {
			volume_id: m.volume_id,
			provider: m.provider,
			change_token: m.change_token,
			last_full_sync_at: m.last_full_sync_at,
			last_incremental_at: m.last_incremental_at,
			consecutive_failures: m.consecutive_failures,
			created_at: m.created_at,
			updated_at: m.updated_at,
		}
	}
}

/// Errors from the cloud-sync-state repository layer.
///
/// Kept thin because the only real failure mode is a database error; the
/// detector-side `Invalidated` / `RateLimited` variants live on
/// [`super::types::ChangeDetectionError`] and should not bubble up here.
#[derive(Error, Debug)]
pub enum CloudSyncStateError {
	#[error("database: {0}")]
	Database(#[from] sea_orm::DbErr),
}

/// CRUD surface the indexer uses to persist cursors and failure diagnostics.
///
/// All methods are async because SeaORM is async; the trait is object-safe
/// (returns concrete futures) so higher layers can inject a mock repository
/// in tests.
#[async_trait]
pub trait CloudSyncStateRepository: Send + Sync {
	/// Fetch the current state for a volume; `None` means never scanned.
	async fn get(&self, volume_id: i32) -> Result<Option<CloudSyncState>, CloudSyncStateError>;

	/// Insert-or-replace the whole row.
	///
	/// Used by first-run code paths that know the full state (e.g. installing
	/// a seed row after `initial_token`). Prefer the targeted helpers
	/// (`update_change_token`, `mark_incremental`, ...) for incremental
	/// updates to avoid clobbering unrelated columns.
	async fn upsert(&self, state: CloudSyncState) -> Result<(), CloudSyncStateError>;

	/// Write just the `change_token` column. Called after every successful
	/// page so an interrupted sync resumes from the last good token.
	async fn update_change_token(
		&self,
		volume_id: i32,
		token: &str,
	) -> Result<(), CloudSyncStateError>;

	/// Record that a full rescan completed; updates `last_full_sync_at` and
	/// resets `consecutive_failures` to zero.
	async fn mark_full_sync_complete(&self, volume_id: i32) -> Result<(), CloudSyncStateError>;

	/// Record that an incremental pass completed; updates
	/// `last_incremental_at` and resets `consecutive_failures`.
	async fn mark_incremental(&self, volume_id: i32) -> Result<(), CloudSyncStateError>;

	/// Bump `consecutive_failures` by one and return the new value so the
	/// scheduler can compute a backoff delay in a single round-trip.
	async fn increment_failures(&self, volume_id: i32) -> Result<i32, CloudSyncStateError>;

	/// Force-reset the failure counter on recovery.
	async fn reset_failures(&self, volume_id: i32) -> Result<(), CloudSyncStateError>;
}

/// Concrete SeaORM-backed implementation.
///
/// Holds a `DatabaseConnection` (`Arc`-cheap-clone) directly because every
/// method body uses it; callers in `CoreContext` wrap this struct in
/// `Arc<dyn CloudSyncStateRepository>`.
pub struct SeaOrmCloudSyncStateRepository {
	db: DatabaseConnection,
}

impl SeaOrmCloudSyncStateRepository {
	/// Construct a repository against an existing library database handle.
	pub fn new(db: DatabaseConnection) -> Self {
		Self { db }
	}

	async fn find_active(
		&self,
		volume_id: i32,
	) -> Result<Option<cloud_sync_state::ActiveModel>, CloudSyncStateError> {
		let model = cloud_sync_state::Entity::find()
			.filter(cloud_sync_state::Column::VolumeId.eq(volume_id))
			.one(&self.db)
			.await?;
		Ok(model.map(Into::into))
	}
}

#[async_trait]
impl CloudSyncStateRepository for SeaOrmCloudSyncStateRepository {
	async fn get(&self, volume_id: i32) -> Result<Option<CloudSyncState>, CloudSyncStateError> {
		let row = cloud_sync_state::Entity::find()
			.filter(cloud_sync_state::Column::VolumeId.eq(volume_id))
			.one(&self.db)
			.await?;
		Ok(row.map(Into::into))
	}

	async fn upsert(&self, state: CloudSyncState) -> Result<(), CloudSyncStateError> {
		// `insert` with `on_conflict` keeps the operation single-statement,
		// avoiding a race between `find` and `insert` when two concurrent
		// passes both believe the row is absent.
		let active = cloud_sync_state::ActiveModel {
			volume_id: Set(state.volume_id),
			provider: Set(state.provider.clone()),
			change_token: Set(state.change_token.clone()),
			last_full_sync_at: Set(state.last_full_sync_at),
			last_incremental_at: Set(state.last_incremental_at),
			consecutive_failures: Set(state.consecutive_failures),
			created_at: Set(state.created_at),
			updated_at: Set(Utc::now()),
		};

		cloud_sync_state::Entity::insert(active)
			.on_conflict(
				sea_orm::sea_query::OnConflict::column(cloud_sync_state::Column::VolumeId)
					.update_columns([
						cloud_sync_state::Column::Provider,
						cloud_sync_state::Column::ChangeToken,
						cloud_sync_state::Column::LastFullSyncAt,
						cloud_sync_state::Column::LastIncrementalAt,
						cloud_sync_state::Column::ConsecutiveFailures,
						cloud_sync_state::Column::UpdatedAt,
					])
					.to_owned(),
			)
			.exec(&self.db)
			.await?;
		Ok(())
	}

	async fn update_change_token(
		&self,
		volume_id: i32,
		token: &str,
	) -> Result<(), CloudSyncStateError> {
		let existing = self.find_active(volume_id).await?.ok_or_else(|| {
			CloudSyncStateError::Database(sea_orm::DbErr::RecordNotFound(format!(
				"cloud_sync_state row missing for volume_id={volume_id}"
			)))
		})?;

		let mut active = existing;
		active.change_token = Set(Some(token.to_string()));
		active.updated_at = Set(Utc::now());
		active.update(&self.db).await?;
		Ok(())
	}

	async fn mark_full_sync_complete(&self, volume_id: i32) -> Result<(), CloudSyncStateError> {
		let existing = self.find_active(volume_id).await?.ok_or_else(|| {
			CloudSyncStateError::Database(sea_orm::DbErr::RecordNotFound(format!(
				"cloud_sync_state row missing for volume_id={volume_id}"
			)))
		})?;

		let mut active = existing;
		let now = Utc::now();
		active.last_full_sync_at = Set(Some(now));
		active.consecutive_failures = Set(0);
		active.updated_at = Set(now);
		active.update(&self.db).await?;
		Ok(())
	}

	async fn mark_incremental(&self, volume_id: i32) -> Result<(), CloudSyncStateError> {
		let existing = self.find_active(volume_id).await?.ok_or_else(|| {
			CloudSyncStateError::Database(sea_orm::DbErr::RecordNotFound(format!(
				"cloud_sync_state row missing for volume_id={volume_id}"
			)))
		})?;

		let mut active = existing;
		let now = Utc::now();
		active.last_incremental_at = Set(Some(now));
		active.consecutive_failures = Set(0);
		active.updated_at = Set(now);
		active.update(&self.db).await?;
		Ok(())
	}

	async fn increment_failures(&self, volume_id: i32) -> Result<i32, CloudSyncStateError> {
		let existing = self.find_active(volume_id).await?.ok_or_else(|| {
			CloudSyncStateError::Database(sea_orm::DbErr::RecordNotFound(format!(
				"cloud_sync_state row missing for volume_id={volume_id}"
			)))
		})?;

		// Reading the current failure count requires the mutable `ActiveModel`
		// to be unwrapped back to the owned value; pulling the model again
		// keeps the logic simple at the cost of one extra round-trip.
		let row = cloud_sync_state::Entity::find()
			.filter(cloud_sync_state::Column::VolumeId.eq(volume_id))
			.one(&self.db)
			.await?
			.ok_or_else(|| {
				CloudSyncStateError::Database(sea_orm::DbErr::RecordNotFound(format!(
					"cloud_sync_state row disappeared between checks for volume_id={volume_id}"
				)))
			})?;

		let next = row.consecutive_failures.saturating_add(1);
		let mut active = existing;
		active.consecutive_failures = Set(next);
		active.updated_at = Set(Utc::now());
		active.update(&self.db).await?;
		Ok(next)
	}

	async fn reset_failures(&self, volume_id: i32) -> Result<(), CloudSyncStateError> {
		let existing = self.find_active(volume_id).await?.ok_or_else(|| {
			CloudSyncStateError::Database(sea_orm::DbErr::RecordNotFound(format!(
				"cloud_sync_state row missing for volume_id={volume_id}"
			)))
		})?;

		let mut active = existing;
		active.consecutive_failures = Set(0);
		active.updated_at = Set(Utc::now());
		active.update(&self.db).await?;
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::infra::db::entities::{device, volume};
	use crate::infra::db::migration::Migrator;
	use sea_orm::{ConnectOptions, Database};
	use sea_orm_migration::MigratorTrait;
	use uuid::Uuid;

	async fn setup_db() -> DatabaseConnection {
		let opts = ConnectOptions::new("sqlite::memory:".to_string());
		let db = Database::connect(opts).await.expect("connect sqlite");
		// Migrator::up runs the full migration chain, which is the only way
		// to ensure every FK target table exists before cloud_sync_state is
		// created. Running individual migrations piecemeal would miss
		// dependencies (devices → volumes → cloud_sync_state).
		Migrator::up(&db, None).await.expect("migrate");
		db
	}

	async fn insert_volume(db: &DatabaseConnection, device_id: Uuid) -> i32 {
		use sea_orm::ActiveValue::NotSet;
		let now = Utc::now();
		let active = volume::ActiveModel {
			id: NotSet,
			uuid: Set(Uuid::new_v4()),
			device_id: Set(device_id),
			fingerprint: Set(format!("fp-{}", Uuid::new_v4())),
			display_name: Set(Some("Test".into())),
			tracked_at: Set(now),
			last_seen_at: Set(now),
			is_online: Set(true),
			total_capacity: Set(None),
			available_capacity: Set(None),
			unique_bytes: Set(None),
			read_speed_mbps: Set(None),
			write_speed_mbps: Set(None),
			last_speed_test_at: Set(None),
			total_file_count: Set(None),
			total_directory_count: Set(None),
			last_indexed_at: Set(None),
			file_system: Set(None),
			mount_point: Set(Some("onedrive://root".into())),
			is_removable: Set(None),
			is_network_drive: Set(Some(true)),
			device_model: Set(None),
			volume_type: Set(Some("Cloud".into())),
			is_user_visible: Set(Some(true)),
			auto_track_eligible: Set(Some(false)),
			cloud_identifier: Set(Some("root".into())),
			cloud_config: Set(None),
		};
		let inserted = active.insert(db).await.expect("insert volume");
		inserted.id
	}

	async fn insert_device(db: &DatabaseConnection) -> Uuid {
		use sea_orm::ActiveValue::NotSet;
		let device_uuid = Uuid::new_v4();
		let active = device::ActiveModel {
			id: NotSet,
			uuid: Set(device_uuid),
			name: Set("test-device".into()),
			slug: Set(format!("slug-{device_uuid}")),
			os: Set("linux".into()),
			os_version: Set(None),
			hardware_model: Set(None),
			cpu_model: Set(None),
			cpu_architecture: Set(None),
			cpu_cores_physical: Set(None),
			cpu_cores_logical: Set(None),
			cpu_frequency_mhz: Set(None),
			memory_total_bytes: Set(None),
			form_factor: Set(None),
			manufacturer: Set(None),
			gpu_models: Set(None),
			boot_disk_type: Set(None),
			boot_disk_capacity_bytes: Set(None),
			swap_total_bytes: Set(None),
			network_addresses: Set(serde_json::json!([])),
			is_online: Set(true),
			last_seen_at: Set(Utc::now()),
			capabilities: Set(serde_json::json!({})),
			created_at: Set(Utc::now()),
			updated_at: Set(Utc::now()),
			sync_enabled: Set(false),
		};
		active.insert(db).await.expect("insert device");
		device_uuid
	}

	#[tokio::test]
	async fn test_get_returns_none_when_absent() {
		let db = setup_db().await;
		let repo = SeaOrmCloudSyncStateRepository::new(db);
		assert!(repo.get(42).await.expect("get").is_none());
	}

	#[tokio::test]
	async fn test_upsert_then_get_round_trips() {
		let db = setup_db().await;
		let device_uuid = insert_device(&db).await;
		let volume_id = insert_volume(&db, device_uuid).await;
		let repo = SeaOrmCloudSyncStateRepository::new(db);

		let mut state = CloudSyncState::new(volume_id, "onedrive");
		state.change_token = Some("token-1".into());
		repo.upsert(state.clone()).await.expect("upsert");

		let fetched = repo.get(volume_id).await.expect("get").expect("row");
		assert_eq!(fetched.volume_id, volume_id);
		assert_eq!(fetched.provider, "onedrive");
		assert_eq!(fetched.change_token.as_deref(), Some("token-1"));
		assert_eq!(fetched.consecutive_failures, 0);
	}

	#[tokio::test]
	async fn test_update_change_token_preserves_other_columns() {
		let db = setup_db().await;
		let device_uuid = insert_device(&db).await;
		let volume_id = insert_volume(&db, device_uuid).await;
		let repo = SeaOrmCloudSyncStateRepository::new(db);

		let mut seed = CloudSyncState::new(volume_id, "onedrive");
		seed.change_token = Some("token-a".into());
		seed.last_full_sync_at = Some(Utc::now());
		repo.upsert(seed).await.unwrap();

		repo.update_change_token(volume_id, "token-b")
			.await
			.unwrap();

		let fetched = repo.get(volume_id).await.unwrap().unwrap();
		assert_eq!(fetched.change_token.as_deref(), Some("token-b"));
		// last_full_sync_at must survive a token-only update — otherwise the
		// scheduler would treat every incremental pass as a first-ever sync.
		assert!(fetched.last_full_sync_at.is_some());
	}

	#[tokio::test]
	async fn test_mark_full_sync_complete_sets_timestamp_and_resets_failures() {
		let db = setup_db().await;
		let device_uuid = insert_device(&db).await;
		let volume_id = insert_volume(&db, device_uuid).await;
		let repo = SeaOrmCloudSyncStateRepository::new(db);

		let mut seed = CloudSyncState::new(volume_id, "onedrive");
		seed.consecutive_failures = 4;
		repo.upsert(seed).await.unwrap();

		repo.mark_full_sync_complete(volume_id).await.unwrap();

		let fetched = repo.get(volume_id).await.unwrap().unwrap();
		assert!(fetched.last_full_sync_at.is_some());
		assert_eq!(fetched.consecutive_failures, 0);
	}

	#[tokio::test]
	async fn test_mark_incremental_sets_timestamp() {
		let db = setup_db().await;
		let device_uuid = insert_device(&db).await;
		let volume_id = insert_volume(&db, device_uuid).await;
		let repo = SeaOrmCloudSyncStateRepository::new(db);

		repo.upsert(CloudSyncState::new(volume_id, "onedrive"))
			.await
			.unwrap();
		repo.mark_incremental(volume_id).await.unwrap();

		let fetched = repo.get(volume_id).await.unwrap().unwrap();
		assert!(fetched.last_incremental_at.is_some());
	}

	#[tokio::test]
	async fn test_increment_and_reset_failures() {
		let db = setup_db().await;
		let device_uuid = insert_device(&db).await;
		let volume_id = insert_volume(&db, device_uuid).await;
		let repo = SeaOrmCloudSyncStateRepository::new(db);

		repo.upsert(CloudSyncState::new(volume_id, "onedrive"))
			.await
			.unwrap();

		assert_eq!(repo.increment_failures(volume_id).await.unwrap(), 1);
		assert_eq!(repo.increment_failures(volume_id).await.unwrap(), 2);
		assert_eq!(repo.increment_failures(volume_id).await.unwrap(), 3);

		repo.reset_failures(volume_id).await.unwrap();
		assert_eq!(
			repo.get(volume_id)
				.await
				.unwrap()
				.unwrap()
				.consecutive_failures,
			0
		);
	}

	#[tokio::test]
	async fn test_cascade_on_volume_delete() {
		let db = setup_db().await;
		let device_uuid = insert_device(&db).await;
		let volume_id = insert_volume(&db, device_uuid).await;
		let repo = SeaOrmCloudSyncStateRepository::new(db.clone());

		repo.upsert(CloudSyncState::new(volume_id, "onedrive"))
			.await
			.unwrap();

		// Deleting the volume must cascade — otherwise orphan sync rows would
		// accumulate on disconnect/reconnect cycles.
		volume::Entity::delete_by_id(volume_id)
			.exec(&db)
			.await
			.unwrap();

		assert!(repo.get(volume_id).await.unwrap().is_none());
	}
}
