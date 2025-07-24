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
                    .table(Volume::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Volume::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Volume::Uuid).text().not_null().unique_key())
                    .col(ColumnDef::new(Volume::Fingerprint).text().not_null())
                    .col(ColumnDef::new(Volume::DisplayName).text())
                    .col(
                        ColumnDef::new(Volume::TrackedAt)
                            .timestamp()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Volume::LastSeenAt)
                            .timestamp()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Volume::IsOnline)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(ColumnDef::new(Volume::TotalCapacity).big_integer())
                    .col(ColumnDef::new(Volume::AvailableCapacity).big_integer())
                    .col(ColumnDef::new(Volume::ReadSpeedMbps).integer())
                    .col(ColumnDef::new(Volume::WriteSpeedMbps).integer())
                    .col(ColumnDef::new(Volume::LastSpeedTestAt).timestamp())
                    .col(ColumnDef::new(Volume::FileSystem).text())
                    .col(ColumnDef::new(Volume::MountPoint).text())
                    .col(ColumnDef::new(Volume::IsRemovable).boolean())
                    .col(ColumnDef::new(Volume::IsNetworkDrive).boolean())
                    .col(ColumnDef::new(Volume::DeviceModel).text())
                    .to_owned(),
            )
            .await?;

        // Create unique index on fingerprint (since each library tracks volumes independently)
        manager
            .create_index(
                Index::create()
                    .name("idx_volume_fingerprint_unique")
                    .table(Volume::Table)
                    .col(Volume::Fingerprint)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Create index on last_seen_at for efficient queries
        manager
            .create_index(
                Index::create()
                    .name("idx_volume_last_seen_at")
                    .table(Volume::Table)
                    .col(Volume::LastSeenAt)
                    .to_owned(),
            )
            .await?;

        // Create index on is_online for filtering
        manager
            .create_index(
                Index::create()
                    .name("idx_volume_is_online")
                    .table(Volume::Table)
                    .col(Volume::IsOnline)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Volume::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Volume {
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