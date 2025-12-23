//! Migration to add file_count column to content_kinds table
//!
//! Adds a file_count column to track the number of content identities for each content kind.
//! This enables efficient statistics calculation without needing to count on every query.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.alter_table(
				Table::alter()
					.table(ContentKinds::Table)
					.add_column(
						ColumnDef::new(ContentKinds::FileCount)
							.big_integer()
							.not_null()
							.default(0),
					)
					.to_owned(),
			)
			.await?;

		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.alter_table(
				Table::alter()
					.table(ContentKinds::Table)
					.drop_column(ContentKinds::FileCount)
					.to_owned(),
			)
			.await?;

		Ok(())
	}
}

#[derive(DeriveIden)]
enum ContentKinds {
	Table,
	FileCount,
}
