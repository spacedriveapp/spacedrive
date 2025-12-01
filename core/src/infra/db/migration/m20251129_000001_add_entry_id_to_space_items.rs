use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
	fn name(&self) -> &str {
		"m20251129_000001_add_entry_id_to_space_items"
	}
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Add entry_id column to space_items table
		// Note: SQLite doesn't enforce foreign keys by default, and doesn't support
		// adding FK constraints via ALTER TABLE. The column is just an integer reference.
		manager
			.alter_table(
				Table::alter()
					.table(SpaceItems::Table)
					.add_column(ColumnDef::new(SpaceItems::EntryId).integer().null())
					.to_owned(),
			)
			.await?;

		// Create index for entry_id lookups
		manager
			.create_index(
				Index::create()
					.name("idx_space_items_entry_id")
					.table(SpaceItems::Table)
					.col(SpaceItems::EntryId)
					.to_owned(),
			)
			.await?;

		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Drop index first
		manager
			.drop_index(
				Index::drop()
					.name("idx_space_items_entry_id")
					.table(SpaceItems::Table)
					.to_owned(),
			)
			.await?;

		// Drop the column (SQLite doesn't support DROP COLUMN directly,
		// but sea-orm-migration handles this)
		manager
			.alter_table(
				Table::alter()
					.table(SpaceItems::Table)
					.drop_column(SpaceItems::EntryId)
					.to_owned(),
			)
			.await?;

		Ok(())
	}
}

#[derive(Iden)]
enum SpaceItems {
	Table,
	EntryId,
}
