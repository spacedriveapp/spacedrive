use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
	fn name(&self) -> &str {
		"m20250103_000001_migrate_space_item_entry_id_to_uuid"
	}
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Delete all space_items that have a non-null entry_id
		// These items used local entry IDs which don't sync correctly across devices
		manager
			.get_connection()
			.execute_unprepared("DELETE FROM space_items WHERE entry_id IS NOT NULL")
			.await?;

		// Drop the old index
		manager
			.drop_index(
				Index::drop()
					.name("idx_space_items_entry_id")
					.table(SpaceItems::Table)
					.to_owned(),
			)
			.await?;

		// Drop the old entry_id column
		manager
			.alter_table(
				Table::alter()
					.table(SpaceItems::Table)
					.drop_column(SpaceItems::EntryId)
					.to_owned(),
			)
			.await?;

		// Add new entry_uuid column
		manager
			.alter_table(
				Table::alter()
					.table(SpaceItems::Table)
					.add_column(ColumnDef::new(SpaceItems::EntryUuid).uuid().null())
					.to_owned(),
			)
			.await?;

		// Create index for entry_uuid lookups
		manager
			.create_index(
				Index::create()
					.name("idx_space_items_entry_uuid")
					.table(SpaceItems::Table)
					.col(SpaceItems::EntryUuid)
					.to_owned(),
			)
			.await?;

		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Drop entry_uuid index
		manager
			.drop_index(
				Index::drop()
					.name("idx_space_items_entry_uuid")
					.table(SpaceItems::Table)
					.to_owned(),
			)
			.await?;

		// Drop entry_uuid column
		manager
			.alter_table(
				Table::alter()
					.table(SpaceItems::Table)
					.drop_column(SpaceItems::EntryUuid)
					.to_owned(),
			)
			.await?;

		// Re-add entry_id column
		manager
			.alter_table(
				Table::alter()
					.table(SpaceItems::Table)
					.add_column(ColumnDef::new(SpaceItems::EntryId).integer().null())
					.to_owned(),
			)
			.await?;

		// Re-create index for entry_id
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
}

#[derive(Iden)]
enum SpaceItems {
	Table,
	EntryId,
	EntryUuid,
}
