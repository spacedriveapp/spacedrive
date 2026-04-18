//! Create `cloud_sync_state` table for provider change-detection cursors.
//!
//! The indexer treats every cloud entry as `Change::New` on every pass, which
//! triggers a full content re-hash of millions of files on large OneDrive /
//! Google Drive / Dropbox volumes. Per-provider delta APIs return only what
//! changed since a prior token, so we persist that token here alongside the
//! diagnostics (`consecutive_failures`, timestamps) that drive the indexer's
//! backoff and resync logic. Keyed on `volume_id` so a cloud volume always
//! has at most one sync state; `ON DELETE CASCADE` keeps cleanup automatic.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.create_table(
				Table::create()
					.table(CloudSyncState::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(CloudSyncState::VolumeId)
							.integer()
							.not_null()
							.primary_key(),
					)
					.col(ColumnDef::new(CloudSyncState::Provider).string().not_null())
					// Nullable: empty until the first successful sync pass
					// records a baseline token from the provider.
					.col(ColumnDef::new(CloudSyncState::ChangeToken).string().null())
					.col(
						ColumnDef::new(CloudSyncState::LastFullSyncAt)
							.timestamp()
							.null(),
					)
					.col(
						ColumnDef::new(CloudSyncState::LastIncrementalAt)
							.timestamp()
							.null(),
					)
					.col(
						ColumnDef::new(CloudSyncState::ConsecutiveFailures)
							.integer()
							.not_null()
							.default(0),
					)
					.col(
						ColumnDef::new(CloudSyncState::CreatedAt)
							.timestamp()
							.not_null()
							.default(Expr::current_timestamp()),
					)
					.col(
						ColumnDef::new(CloudSyncState::UpdatedAt)
							.timestamp()
							.not_null()
							.default(Expr::current_timestamp()),
					)
					.foreign_key(
						ForeignKey::create()
							.name("fk_cloud_sync_state_volume_id")
							.from(CloudSyncState::Table, CloudSyncState::VolumeId)
							.to(Volumes::Table, Volumes::Id)
							.on_delete(ForeignKeyAction::Cascade),
					)
					.to_owned(),
			)
			.await?;

		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.drop_table(Table::drop().table(CloudSyncState::Table).to_owned())
			.await
	}
}

#[derive(DeriveIden)]
enum CloudSyncState {
	Table,
	VolumeId,
	Provider,
	ChangeToken,
	LastFullSyncAt,
	LastIncrementalAt,
	ConsecutiveFailures,
	CreatedAt,
	UpdatedAt,
}

#[derive(DeriveIden)]
enum Volumes {
	Table,
	Id,
}
