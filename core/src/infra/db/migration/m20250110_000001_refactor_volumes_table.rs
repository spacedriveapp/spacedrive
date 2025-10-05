use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// For SQLite, we can't easily alter columns, so we'll just add the UUID column
		// if the table exists with the old schema

		// Try to add UUID column to existing table
		let _ = manager
			.alter_table(
				Table::alter()
					.table(Volumes::Table)
					.add_column_if_not_exists(
						ColumnDef::new(Volumes::Uuid)
							.string() // SQLite doesn't have native UUID type
							.not_null()
							.default(""), // Will be populated later
					)
					.to_owned(),
			)
			.await;

		// Add other missing columns one by one (SQLite limitation)
		let _ = manager
			.alter_table(
				Table::alter()
					.table(Volumes::Table)
					.add_column_if_not_exists(ColumnDef::new(Volumes::Fingerprint).string())
					.to_owned(),
			)
			.await;

		let _ = manager
			.alter_table(
				Table::alter()
					.table(Volumes::Table)
					.add_column_if_not_exists(ColumnDef::new(Volumes::DisplayName).string())
					.to_owned(),
			)
			.await;

		let _ = manager
			.alter_table(
				Table::alter()
					.table(Volumes::Table)
					.add_column_if_not_exists(
						ColumnDef::new(Volumes::TrackedAt)
							.timestamp()
							.not_null()
							.default(Expr::current_timestamp()),
					)
					.to_owned(),
			)
			.await;

		let _ = manager
			.alter_table(
				Table::alter()
					.table(Volumes::Table)
					.add_column_if_not_exists(ColumnDef::new(Volumes::LastSpeedTestAt).timestamp())
					.to_owned(),
			)
			.await;

		let _ = manager
			.alter_table(
				Table::alter()
					.table(Volumes::Table)
					.add_column_if_not_exists(ColumnDef::new(Volumes::ReadSpeedMbps).integer())
					.to_owned(),
			)
			.await;

		let _ = manager
			.alter_table(
				Table::alter()
					.table(Volumes::Table)
					.add_column_if_not_exists(ColumnDef::new(Volumes::WriteSpeedMbps).integer())
					.to_owned(),
			)
			.await;

		let _ = manager
			.alter_table(
				Table::alter()
					.table(Volumes::Table)
					.add_column_if_not_exists(
						ColumnDef::new(Volumes::IsOnline).boolean().default(true),
					)
					.to_owned(),
			)
			.await;

		let _ = manager
			.alter_table(
				Table::alter()
					.table(Volumes::Table)
					.add_column_if_not_exists(ColumnDef::new(Volumes::IsNetworkDrive).boolean())
					.to_owned(),
			)
			.await;

		let _ = manager
			.alter_table(
				Table::alter()
					.table(Volumes::Table)
					.add_column_if_not_exists(ColumnDef::new(Volumes::DeviceModel).string())
					.to_owned(),
			)
			.await;

		let _ = manager
			.alter_table(
				Table::alter()
					.table(Volumes::Table)
					.add_column_if_not_exists(ColumnDef::new(Volumes::VolumeType).string())
					.to_owned(),
			)
			.await;

		let _ = manager
			.alter_table(
				Table::alter()
					.table(Volumes::Table)
					.add_column_if_not_exists(ColumnDef::new(Volumes::IsUserVisible).boolean())
					.to_owned(),
			)
			.await;

		let _ = manager
			.alter_table(
				Table::alter()
					.table(Volumes::Table)
					.add_column_if_not_exists(ColumnDef::new(Volumes::AutoTrackEligible).boolean())
					.to_owned(),
			)
			.await;

		let _ = manager
			.alter_table(
				Table::alter()
					.table(Volumes::Table)
					.add_column_if_not_exists(
						ColumnDef::new(Volumes::LastSeenAt)
							.timestamp()
							.not_null()
							.default(Expr::current_timestamp()),
					)
					.to_owned(),
			)
			.await;

		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Remove added columns
		// Note: SQLite doesn't support dropping columns easily
		Ok(())
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
	MountPoint,
	TotalCapacity,
	AvailableCapacity,
	ReadSpeedMbps,
	WriteSpeedMbps,
	IsRemovable,
	IsEjectable,
	IsOnline,
	IsNetworkDrive,
	FileSystemType,
	DeviceModel,
	VolumeType,
	IsUserVisible,
	AutoTrackEligible,
	TrackedAt,
	LastSeenAt,
	LastSpeedTestAt,
	CreatedAt,
	UpdatedAt,
}
