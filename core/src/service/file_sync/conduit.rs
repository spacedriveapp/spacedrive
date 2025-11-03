use crate::infra::db::entities::{entry, sync_conduit, sync_generation};
use anyhow::Result;
use chrono::Utc;
use sea_orm::{prelude::*, ActiveValue::Set, DatabaseConnection, QueryOrder};
use std::sync::Arc;
use uuid::Uuid;

/// Manages sync conduit CRUD operations
pub struct ConduitManager {
	db: Arc<DatabaseConnection>,
}

impl ConduitManager {
	pub fn new(db: Arc<DatabaseConnection>) -> Self {
		Self { db }
	}

	/// Create a new sync conduit
	pub async fn create_conduit(
		&self,
		source_entry_id: i32,
		target_entry_id: i32,
		mode: sync_conduit::SyncMode,
		schedule: String,
	) -> Result<sync_conduit::Model> {
		// Validate entries exist and are directories
		let source = entry::Entity::find_by_id(source_entry_id)
			.one(&*self.db)
			.await?
			.ok_or_else(|| anyhow::anyhow!("Source entry not found"))?;

		let target = entry::Entity::find_by_id(target_entry_id)
			.one(&*self.db)
			.await?
			.ok_or_else(|| anyhow::anyhow!("Target entry not found"))?;

		if source.kind != 1 || target.kind != 1 {
			return Err(anyhow::anyhow!(
				"Both source and target must be directories"
			));
		}

		// Check for duplicate conduits
		let existing = sync_conduit::Entity::find()
			.filter(sync_conduit::Column::SourceEntryId.eq(source_entry_id))
			.filter(sync_conduit::Column::TargetEntryId.eq(target_entry_id))
			.one(&*self.db)
			.await?;

		if existing.is_some() {
			return Err(anyhow::anyhow!(
				"Conduit already exists between these entries"
			));
		}

		// Create conduit
		let now = Utc::now();
		let conduit = sync_conduit::ActiveModel {
			uuid: Set(Uuid::new_v4()),
			source_entry_id: Set(source_entry_id),
			target_entry_id: Set(target_entry_id),
			sync_mode: Set(mode.as_str().to_string()),
			enabled: Set(true),
			schedule: Set(schedule),
			use_index_rules: Set(true),
			index_mode_override: Set(None),
			parallel_transfers: Set(3),
			bandwidth_limit_mbps: Set(None),
			last_sync_completed_at: Set(None),
			sync_generation: Set(0),
			last_sync_error: Set(None),
			total_syncs: Set(0),
			files_synced: Set(0),
			bytes_transferred: Set(0),
			created_at: Set(now),
			updated_at: Set(now),
			..Default::default()
		};

		let result = conduit.insert(&*self.db).await?;

		Ok(result)
	}

	/// Get conduit by ID
	pub async fn get_conduit(&self, id: i32) -> Result<sync_conduit::Model> {
		sync_conduit::Entity::find_by_id(id)
			.one(&*self.db)
			.await?
			.ok_or_else(|| anyhow::anyhow!("Conduit not found"))
	}

	/// Get conduit by UUID
	pub async fn get_conduit_by_uuid(&self, uuid: Uuid) -> Result<sync_conduit::Model> {
		sync_conduit::Entity::find()
			.filter(sync_conduit::Column::Uuid.eq(uuid))
			.one(&*self.db)
			.await?
			.ok_or_else(|| anyhow::anyhow!("Conduit not found"))
	}

	/// List all conduits
	pub async fn list_all(&self) -> Result<Vec<sync_conduit::Model>> {
		Ok(sync_conduit::Entity::find().all(&*self.db).await?)
	}

	/// List enabled conduits
	pub async fn list_enabled(&self) -> Result<Vec<sync_conduit::Model>> {
		Ok(sync_conduit::Entity::find()
			.filter(sync_conduit::Column::Enabled.eq(true))
			.all(&*self.db)
			.await?)
	}

	/// Update conduit enabled status
	pub async fn set_enabled(&self, conduit_id: i32, enabled: bool) -> Result<()> {
		let conduit = self.get_conduit(conduit_id).await?;

		let mut active: sync_conduit::ActiveModel = conduit.into();
		active.enabled = Set(enabled);
		active.updated_at = Set(Utc::now());

		active.update(&*self.db).await?;

		Ok(())
	}

	/// Update conduit after successful sync
	pub async fn update_after_sync(&self, conduit_id: i32) -> Result<()> {
		let conduit = self.get_conduit(conduit_id).await?;

		let mut active: sync_conduit::ActiveModel = conduit.into();
		active.last_sync_completed_at = Set(Some(Utc::now()));
		active.sync_generation = Set(active.sync_generation.unwrap() + 1);
		active.total_syncs = Set(active.total_syncs.unwrap() + 1);
		active.updated_at = Set(Utc::now());
		active.last_sync_error = Set(None);

		active.update(&*self.db).await?;

		Ok(())
	}

	/// Record sync error
	pub async fn record_sync_error(&self, conduit_id: i32, error: String) -> Result<()> {
		let conduit = self.get_conduit(conduit_id).await?;

		let mut active: sync_conduit::ActiveModel = conduit.into();
		active.last_sync_error = Set(Some(error));
		active.updated_at = Set(Utc::now());

		active.update(&*self.db).await?;

		Ok(())
	}

	/// Create new generation record
	pub async fn create_generation(
		&self,
		conduit_id: i32,
		generation: i64,
	) -> Result<sync_generation::Model> {
		let gen = sync_generation::ActiveModel {
			conduit_id: Set(conduit_id),
			generation: Set(generation),
			started_at: Set(Utc::now()),
			completed_at: Set(None),
			files_copied: Set(0),
			files_deleted: Set(0),
			conflicts_resolved: Set(0),
			bytes_transferred: Set(0),
			errors_encountered: Set(0),
			verified_at: Set(None),
			verification_status: Set("unverified".to_string()),
			..Default::default()
		};

		Ok(gen.insert(&*self.db).await?)
	}

	/// Mark generation as complete
	pub async fn complete_generation(&self, generation_id: i32) -> Result<()> {
		let gen = sync_generation::Entity::find_by_id(generation_id)
			.one(&*self.db)
			.await?
			.ok_or_else(|| anyhow::anyhow!("Generation not found"))?;

		let mut active: sync_generation::ActiveModel = gen.into();
		active.completed_at = Set(Some(Utc::now()));

		active.update(&*self.db).await?;

		Ok(())
	}

	/// Update generation verification status
	pub async fn update_verification_status(
		&self,
		generation_id: i32,
		status: &str,
	) -> Result<()> {
		let gen = sync_generation::Entity::find_by_id(generation_id)
			.one(&*self.db)
			.await?
			.ok_or_else(|| anyhow::anyhow!("Generation not found"))?;

		let mut active: sync_generation::ActiveModel = gen.into();
		active.verification_status = Set(status.to_string());

		if status == "verified" {
			active.verified_at = Set(Some(Utc::now()));
		}

		active.update(&*self.db).await?;

		Ok(())
	}

	/// Get last completed generation for a conduit
	pub async fn get_last_completed_generation(
		&self,
		conduit_id: i32,
	) -> Result<Option<sync_generation::Model>> {
		Ok(sync_generation::Entity::find()
			.filter(sync_generation::Column::ConduitId.eq(conduit_id))
			.filter(sync_generation::Column::CompletedAt.is_not_null())
			.order_by_desc(sync_generation::Column::Generation)
			.one(&*self.db)
			.await?)
	}

	/// Get entry by ID (helper)
	pub async fn get_entry(&self, entry_id: i32) -> Result<entry::Model> {
		entry::Entity::find_by_id(entry_id)
			.one(&*self.db)
			.await?
			.ok_or_else(|| anyhow::anyhow!("Entry not found"))
	}

	/// Delete a conduit
	pub async fn delete_conduit(&self, conduit_id: i32) -> Result<()> {
		sync_conduit::Entity::delete_by_id(conduit_id)
			.exec(&*self.db)
			.await?;

		Ok(())
	}
}
