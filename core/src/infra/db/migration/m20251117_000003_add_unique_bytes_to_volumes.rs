//! Add unique_bytes column to volumes table for content deduplication tracking

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Add unique_bytes column to volumes table
		manager
			.alter_table(
				Table::alter()
					.table(Volumes::Table)
					.add_column(ColumnDef::new(Volumes::UniqueBytes).big_integer().null())
					.to_owned(),
			)
			.await?;

		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Remove unique_bytes column
		manager
			.alter_table(
				Table::alter()
					.table(Volumes::Table)
					.drop_column(Volumes::UniqueBytes)
					.to_owned(),
			)
			.await?;

		Ok(())
	}
}

#[derive(Iden)]
enum Volumes {
	Table,
	UniqueBytes,
}
