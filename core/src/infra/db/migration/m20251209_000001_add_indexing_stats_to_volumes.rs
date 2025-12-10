//! Add ephemeral indexing statistics to volumes table

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Add total_file_count column
		manager
			.alter_table(
				Table::alter()
					.table(Volumes::Table)
					.add_column(
						ColumnDef::new(Volumes::TotalFileCount)
							.big_integer()
							.null(),
					)
					.to_owned(),
			)
			.await?;

		// Add total_directory_count column
		manager
			.alter_table(
				Table::alter()
					.table(Volumes::Table)
					.add_column(
						ColumnDef::new(Volumes::TotalDirectoryCount)
							.big_integer()
							.null(),
					)
					.to_owned(),
			)
			.await?;

		// Add last_indexed_at column
		manager
			.alter_table(
				Table::alter()
					.table(Volumes::Table)
					.add_column(
						ColumnDef::new(Volumes::LastIndexedAt)
							.date_time()
							.null(),
					)
					.to_owned(),
			)
			.await?;

		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Remove last_indexed_at column
		manager
			.alter_table(
				Table::alter()
					.table(Volumes::Table)
					.drop_column(Volumes::LastIndexedAt)
					.to_owned(),
			)
			.await?;

		// Remove total_directory_count column
		manager
			.alter_table(
				Table::alter()
					.table(Volumes::Table)
					.drop_column(Volumes::TotalDirectoryCount)
					.to_owned(),
			)
			.await?;

		// Remove total_file_count column
		manager
			.alter_table(
				Table::alter()
					.table(Volumes::Table)
					.drop_column(Volumes::TotalFileCount)
					.to_owned(),
			)
			.await?;

		Ok(())
	}
}

#[derive(Iden)]
enum Volumes {
	Table,
	TotalFileCount,
	TotalDirectoryCount,
	LastIndexedAt,
}
