use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.create_table(
				Table::create()
					.table(DeviceStateTombstones::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(DeviceStateTombstones::Id)
							.integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(
						ColumnDef::new(DeviceStateTombstones::ModelType)
							.string()
							.not_null(),
					)
					.col(
						ColumnDef::new(DeviceStateTombstones::RecordUuid)
							.uuid()
							.not_null(),
					)
					.col(
						ColumnDef::new(DeviceStateTombstones::DeviceId)
							.integer()
							.not_null(),
					)
					.col(
						ColumnDef::new(DeviceStateTombstones::DeletedAt)
							.timestamp_with_time_zone()
							.not_null(),
					)
					.foreign_key(
						ForeignKey::create()
							.name("fk_tombstone_device")
							.from(DeviceStateTombstones::Table, DeviceStateTombstones::DeviceId)
							.to(Devices::Table, Devices::Id)
							.on_delete(ForeignKeyAction::Cascade),
					)
					.to_owned(),
			)
			.await?;

		// Unique constraint on (model_type, record_uuid, device_id)
		manager
			.create_index(
				Index::create()
					.name("idx_tombstones_unique")
					.table(DeviceStateTombstones::Table)
					.col(DeviceStateTombstones::ModelType)
					.col(DeviceStateTombstones::RecordUuid)
					.col(DeviceStateTombstones::DeviceId)
					.unique()
					.to_owned(),
			)
			.await?;

		// Index for lookup by model_type, device_id, deleted_at
		manager
			.create_index(
				Index::create()
					.name("idx_tombstones_lookup")
					.table(DeviceStateTombstones::Table)
					.col(DeviceStateTombstones::ModelType)
					.col(DeviceStateTombstones::DeviceId)
					.col(DeviceStateTombstones::DeletedAt)
					.to_owned(),
			)
			.await?;

		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.drop_table(Table::drop().table(DeviceStateTombstones::Table).to_owned())
			.await?;

		Ok(())
	}
}

#[derive(DeriveIden)]
enum DeviceStateTombstones {
	Table,
	Id,
	ModelType,
	RecordUuid,
	DeviceId,
	DeletedAt,
}

#[derive(DeriveIden)]
enum Devices {
	Table,
	Id,
}
