use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Create sync_conduit table
		manager
			.create_table(
				Table::create()
					.table(SyncConduit::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(SyncConduit::Id)
							.integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(ColumnDef::new(SyncConduit::Uuid).binary().not_null().unique_key())
					.col(ColumnDef::new(SyncConduit::SourceEntryId).integer().not_null())
					.col(ColumnDef::new(SyncConduit::TargetEntryId).integer().not_null())
					.col(ColumnDef::new(SyncConduit::SyncMode).string().not_null())
					.col(
						ColumnDef::new(SyncConduit::Enabled)
							.boolean()
							.not_null()
							.default(true),
					)
					.col(
						ColumnDef::new(SyncConduit::Schedule)
							.string()
							.not_null()
							.default("manual"),
					)
					.col(
						ColumnDef::new(SyncConduit::UseIndexRules)
							.boolean()
							.not_null()
							.default(true),
					)
					.col(ColumnDef::new(SyncConduit::IndexModeOverride).string())
					.col(
						ColumnDef::new(SyncConduit::ParallelTransfers)
							.integer()
							.not_null()
							.default(3),
					)
					.col(ColumnDef::new(SyncConduit::BandwidthLimitMbps).integer())
					.col(ColumnDef::new(SyncConduit::LastSyncCompletedAt).timestamp_with_time_zone())
					.col(
						ColumnDef::new(SyncConduit::SyncGeneration)
							.big_integer()
							.not_null()
							.default(0),
					)
					.col(ColumnDef::new(SyncConduit::LastSyncError).string())
					.col(
						ColumnDef::new(SyncConduit::TotalSyncs)
							.big_integer()
							.not_null()
							.default(0),
					)
					.col(
						ColumnDef::new(SyncConduit::FilesSynced)
							.big_integer()
							.not_null()
							.default(0),
					)
					.col(
						ColumnDef::new(SyncConduit::BytesTransferred)
							.big_integer()
							.not_null()
							.default(0),
					)
					.col(
						ColumnDef::new(SyncConduit::CreatedAt)
							.timestamp_with_time_zone()
							.not_null(),
					)
					.col(
						ColumnDef::new(SyncConduit::UpdatedAt)
							.timestamp_with_time_zone()
							.not_null(),
					)
					.foreign_key(
						ForeignKey::create()
							.name("fk_sync_conduit_source_entry")
							.from(SyncConduit::Table, SyncConduit::SourceEntryId)
							.to(Entry::Table, Entry::Id)
							.on_delete(ForeignKeyAction::Cascade),
					)
					.foreign_key(
						ForeignKey::create()
							.name("fk_sync_conduit_target_entry")
							.from(SyncConduit::Table, SyncConduit::TargetEntryId)
							.to(Entry::Table, Entry::Id)
							.on_delete(ForeignKeyAction::Cascade),
					)
					.to_owned(),
			)
			.await?;

		// Create index on enabled column for active conduit queries
		manager
			.create_index(
				Index::create()
					.name("idx_sync_conduit_enabled")
					.table(SyncConduit::Table)
					.col(SyncConduit::Enabled)
					.to_owned(),
			)
			.await?;

		// Create sync_generation table
		manager
			.create_table(
				Table::create()
					.table(SyncGeneration::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(SyncGeneration::Id)
							.integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(ColumnDef::new(SyncGeneration::ConduitId).integer().not_null())
					.col(ColumnDef::new(SyncGeneration::Generation).big_integer().not_null())
					.col(
						ColumnDef::new(SyncGeneration::StartedAt)
							.timestamp_with_time_zone()
							.not_null(),
					)
					.col(ColumnDef::new(SyncGeneration::CompletedAt).timestamp_with_time_zone())
					.col(
						ColumnDef::new(SyncGeneration::FilesCopied)
							.integer()
							.not_null()
							.default(0),
					)
					.col(
						ColumnDef::new(SyncGeneration::FilesDeleted)
							.integer()
							.not_null()
							.default(0),
					)
					.col(
						ColumnDef::new(SyncGeneration::ConflictsResolved)
							.integer()
							.not_null()
							.default(0),
					)
					.col(
						ColumnDef::new(SyncGeneration::BytesTransferred)
							.big_integer()
							.not_null()
							.default(0),
					)
					.col(
						ColumnDef::new(SyncGeneration::ErrorsEncountered)
							.integer()
							.not_null()
							.default(0),
					)
					.col(ColumnDef::new(SyncGeneration::VerifiedAt).timestamp_with_time_zone())
					.col(
						ColumnDef::new(SyncGeneration::VerificationStatus)
							.string()
							.not_null()
							.default("unverified"),
					)
					.foreign_key(
						ForeignKey::create()
							.name("fk_sync_generation_conduit")
							.from(SyncGeneration::Table, SyncGeneration::ConduitId)
							.to(SyncConduit::Table, SyncConduit::Id)
							.on_delete(ForeignKeyAction::Cascade),
					)
					.to_owned(),
			)
			.await?;

		// Create index on (conduit_id, generation) for efficient generation lookups
		manager
			.create_index(
				Index::create()
					.name("idx_sync_generation_conduit")
					.table(SyncGeneration::Table)
					.col(SyncGeneration::ConduitId)
					.col(SyncGeneration::Generation)
					.to_owned(),
			)
			.await?;

		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Drop tables in reverse order (child tables first)
		manager
			.drop_table(Table::drop().table(SyncGeneration::Table).to_owned())
			.await?;

		manager
			.drop_table(Table::drop().table(SyncConduit::Table).to_owned())
			.await?;

		Ok(())
	}
}

#[derive(DeriveIden)]
enum SyncConduit {
	Table,
	Id,
	Uuid,
	SourceEntryId,
	TargetEntryId,
	SyncMode,
	Enabled,
	Schedule,
	UseIndexRules,
	IndexModeOverride,
	ParallelTransfers,
	BandwidthLimitMbps,
	LastSyncCompletedAt,
	SyncGeneration,
	LastSyncError,
	TotalSyncs,
	FilesSynced,
	BytesTransferred,
	CreatedAt,
	UpdatedAt,
}

#[derive(DeriveIden)]
enum SyncGeneration {
	Table,
	Id,
	ConduitId,
	Generation,
	StartedAt,
	CompletedAt,
	FilesCopied,
	FilesDeleted,
	ConflictsResolved,
	BytesTransferred,
	ErrorsEncountered,
	VerifiedAt,
	VerificationStatus,
}

#[derive(DeriveIden)]
enum Entry {
	Table,
	Id,
}
