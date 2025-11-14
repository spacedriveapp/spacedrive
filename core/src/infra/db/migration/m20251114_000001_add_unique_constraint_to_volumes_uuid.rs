use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Add unique index on volumes.uuid to enable ON CONFLICT (uuid) in sync code
		// SQLite treats UNIQUE INDEX the same as UNIQUE constraint for ON CONFLICT purposes
		manager
			.create_index(
				Index::create()
					.if_not_exists()
					.name("idx_volumes_uuid_unique")
					.table(Volumes::Table)
					.col(Volumes::Uuid)
					.unique()
					.to_owned(),
			)
			.await?;

		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Drop the unique index
		manager
			.drop_index(
				Index::drop()
					.name("idx_volumes_uuid_unique")
					.table(Volumes::Table)
					.to_owned(),
			)
			.await?;

		Ok(())
	}
}

#[derive(DeriveIden)]
enum Volumes {
	Table,
	Uuid,
}

