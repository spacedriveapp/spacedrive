//! A source reports how many distinct sets of bytes it holds.
//!
//! `record_count` counts rows and `unique_bytes` counts bytes, so neither
//! answers how many separate files a drive actually contains once copies are
//! folded together. The store knows, because `content` is one row per identity;
//! this is where it says so.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.alter_table(
				Table::alter()
					.table(Sources::Table)
					.add_column(ColumnDef::new(Sources::ContentCount).big_integer().null())
					.to_owned(),
			)
			.await
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.alter_table(
				Table::alter()
					.table(Sources::Table)
					.drop_column(Sources::ContentCount)
					.to_owned(),
			)
			.await
	}
}

#[derive(DeriveIden)]
enum Sources {
	Table,
	ContentCount,
}
