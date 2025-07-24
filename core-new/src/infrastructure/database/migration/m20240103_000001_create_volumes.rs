//! Create volumes table for tracking mounted volumes in each library

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create volumes table
        manager
            .create_table(
                Table::create()
                    .table(Volumes::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Volumes::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Volumes::Uuid).text().not_null().unique_key())
                    .col(ColumnDef::new(Volumes::Fingerprint).text().not_null())
                    .col(ColumnDef::new(Volumes::DisplayName).text())
                    .col(
                        ColumnDef::new(Volumes::TrackedAt)
                            .timestamp()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Volumes::LastSeenAt)
                            .timestamp()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Volumes::IsOnline)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(ColumnDef::new(Volumes::TotalCapacity).big_integer())
                    .col(ColumnDef::new(Volumes::AvailableCapacity).big_integer())
                    .col(ColumnDef::new(Volumes::ReadSpeedMbps).integer())
                    .col(ColumnDef::new(Volumes::WriteSpeedMbps).integer())
                    .col(ColumnDef::new(Volumes::LastSpeedTestAt).timestamp())
                    .col(ColumnDef::new(Volumes::FileSystem).text())
                    .col(ColumnDef::new(Volumes::MountPoint).text())
                    .col(ColumnDef::new(Volumes::IsRemovable).boolean())
                    .col(ColumnDef::new(Volumes::IsNetworkDrive).boolean())
                    .col(ColumnDef::new(Volumes::DeviceModel).text())
                    .to_owned(),
            )
            .await?;

        // Create unique index on fingerprint (since each library tracks volumes independently)
        manager
            .create_index(
                Index::create()
                    .name("idx_volume_fingerprint_unique")
                    .table(Volumes::Table)
                    .col(Volumes::Fingerprint)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Create index on last_seen_at for efficient queries
        manager
            .create_index(
                Index::create()
                    .name("idx_volume_last_seen_at")
                    .table(Volumes::Table)
                    .col(Volumes::LastSeenAt)
                    .to_owned(),
            )
            .await?;

        // Create index on is_online for filtering
        manager
            .create_index(
                Index::create()
                    .name("idx_volume_is_online")
                    .table(Volumes::Table)
                    .col(Volumes::IsOnline)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Volumes::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Volumes {
    Table,
    Id,
    Uuid,
    Fingerprint,
    DisplayName,
    TrackedAt,
    LastSeenAt,
    IsOnline,
    TotalCapacity,
    AvailableCapacity,
    ReadSpeedMbps,
    WriteSpeedMbps,
    LastSpeedTestAt,
    FileSystem,
    MountPoint,
    IsRemovable,
    IsNetworkDrive,
    DeviceModel,
}