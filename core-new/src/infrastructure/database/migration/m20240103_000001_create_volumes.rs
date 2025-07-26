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
					.col(ColumnDef::new(Volumes::DeviceId).text().not_null()) // Device this volume belongs to
					.col(ColumnDef::new(Volumes::Fingerprint).text().not_null())
					.col(ColumnDef::new(Volumes::DisplayName).text())
					.col(ColumnDef::new(Volumes::TrackedAt).timestamp().not_null())
					.col(ColumnDef::new(Volumes::LastSeenAt).timestamp().not_null())
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
					// Volume classification fields
					.col(
						ColumnDef::new(Volumes::VolumeType)
							.text()
							.not_null()
							.default("Unknown"),
					)
					.col(
						ColumnDef::new(Volumes::IsUserVisible)
							.boolean()
							.not_null()
							.default(true),
					)
					.col(
						ColumnDef::new(Volumes::AutoTrackEligible)
							.boolean()
							.not_null()
							.default(false),
					)
					.foreign_key(
						ForeignKey::create()
							.name("fk_volumes_device_id")
							.from(Volumes::Table, Volumes::DeviceId)
							.to(Devices::Table, Devices::Uuid)
							.on_delete(ForeignKeyAction::Cascade)
							.on_update(ForeignKeyAction::Cascade),
					)
					.to_owned(),
			)
			.await?;

		// Create unique index on device_id + fingerprint (volumes are unique per device)
		manager
			.create_index(
				Index::create()
					.name("idx_volume_device_fingerprint_unique")
					.table(Volumes::Table)
					.col(Volumes::DeviceId)
					.col(Volumes::Fingerprint)
					.unique()
					.to_owned(),
			)
			.await?;

		// Create index on device_id for efficient device queries
		manager
			.create_index(
				Index::create()
					.name("idx_volume_device_id")
					.table(Volumes::Table)
					.col(Volumes::DeviceId)
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
	DeviceId,
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
	// Volume classification fields
	VolumeType,
	IsUserVisible,
	AutoTrackEligible,
}

#[derive(DeriveIden)]
enum Devices {
	Table,
	Uuid,
}
