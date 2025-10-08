//! Sync log database migrations
//!
//! Since the sync log lives in a separate database, it has its own
//! migration system independent of the main library database.

use sea_orm_migration::prelude::*;

/// Migrator for sync log database
pub struct SyncLogMigrator;

#[async_trait::async_trait]
impl MigratorTrait for SyncLogMigrator {
	fn migrations() -> Vec<Box<dyn MigrationTrait>> {
		vec![Box::new(InitialSyncLogSchema)]
	}
}

/// Initial sync log schema migration
#[derive(DeriveMigrationName)]
pub struct InitialSyncLogSchema;

#[async_trait::async_trait]
impl MigrationTrait for InitialSyncLogSchema {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Create sync_log table
		manager
			.create_table(
				Table::create()
					.table(SyncLog::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(SyncLog::Id)
							.integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(
						ColumnDef::new(SyncLog::Sequence)
							.big_integer()
							.not_null()
							.unique_key(),
					)
					.col(ColumnDef::new(SyncLog::DeviceId).uuid().not_null())
					.col(
						ColumnDef::new(SyncLog::Timestamp)
							.timestamp_with_time_zone()
							.not_null(),
					)
					.col(ColumnDef::new(SyncLog::ModelType).string().not_null())
					.col(ColumnDef::new(SyncLog::RecordId).uuid().not_null())
					.col(ColumnDef::new(SyncLog::ChangeType).string().not_null())
					.col(
						ColumnDef::new(SyncLog::Version)
							.big_integer()
							.not_null()
							.default(1),
					)
					.col(ColumnDef::new(SyncLog::Data).text().not_null())
					.to_owned(),
			)
			.await?;

		// Create index on sequence (primary lookup for sync)
		manager
			.create_index(
				Index::create()
					.name("idx_sync_log_sequence")
					.table(SyncLog::Table)
					.col(SyncLog::Sequence)
					.to_owned(),
			)
			.await?;

		// Create index on device_id (filter by originating device)
		manager
			.create_index(
				Index::create()
					.name("idx_sync_log_device")
					.table(SyncLog::Table)
					.col(SyncLog::DeviceId)
					.to_owned(),
			)
			.await?;

		// Create composite index on model_type and record_id
		// (find changes to specific records)
		manager
			.create_index(
				Index::create()
					.name("idx_sync_log_model_record")
					.table(SyncLog::Table)
					.col(SyncLog::ModelType)
					.col(SyncLog::RecordId)
					.to_owned(),
			)
			.await?;

		// Create index on timestamp (for vacuum operations)
		manager
			.create_index(
				Index::create()
					.name("idx_sync_log_timestamp")
					.table(SyncLog::Table)
					.col(SyncLog::Timestamp)
					.to_owned(),
			)
			.await?;

		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.drop_table(Table::drop().table(SyncLog::Table).to_owned())
			.await
	}
}

/// Sync log table identifier
#[derive(DeriveIden)]
enum SyncLog {
	Table,
	Id,
	Sequence,
	DeviceId,
	Timestamp,
	ModelType,
	RecordId,
	ChangeType,
	Version,
	Data,
}
