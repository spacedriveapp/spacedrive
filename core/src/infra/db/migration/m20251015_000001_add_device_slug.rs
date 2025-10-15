//! Add unique slug column to devices table for unified addressing

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
	fn name(&self) -> &str {
		"m20251015_000001_add_device_slug"
	}
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// SQLite doesn't support adding a UNIQUE column in ALTER TABLE
		// We need to add the column first, then create a unique index
		manager
			.alter_table(
				Table::alter()
					.table(Devices::Table)
					.add_column(
						ColumnDef::new(Devices::Slug)
							.string()
							.not_null()
							.default(""),
					)
					.to_owned(),
			)
			.await?;

		// Create unique index on slug column
		manager
			.create_index(
				Index::create()
					.name("idx_devices_slug_unique")
					.table(Devices::Table)
					.col(Devices::Slug)
					.unique()
					.to_owned(),
			)
			.await
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Drop index first
		manager
			.drop_index(
				Index::drop()
					.name("idx_devices_slug_unique")
					.table(Devices::Table)
					.to_owned(),
			)
			.await?;

		// Then drop column
		manager
			.alter_table(
				Table::alter()
					.table(Devices::Table)
					.drop_column(Devices::Slug)
					.to_owned(),
			)
			.await
	}
}

#[derive(Iden)]
enum Devices {
	Table,
	Slug,
}
