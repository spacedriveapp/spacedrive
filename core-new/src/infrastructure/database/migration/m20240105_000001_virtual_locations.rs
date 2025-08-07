//! Virtual locations migration - removes relative_path and adds directory_paths table

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Create directory_paths table
		manager
			.create_table(
				Table::create()
					.table(DirectoryPaths::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(DirectoryPaths::EntryId)
							.integer()
							.primary_key(),
					)
					.col(
						ColumnDef::new(DirectoryPaths::Path)
							.text()
							.not_null(),
					)
					.foreign_key(
						ForeignKey::create()
							.name("fk_directory_path_entry")
							.from(DirectoryPaths::Table, DirectoryPaths::EntryId)
							.to(Entries::Table, Entries::Id)
							.on_delete(ForeignKeyAction::Cascade),
					)
					.to_owned(),
			)
			.await?;

		// Create index on path for efficient lookups
		manager
			.create_index(
				Index::create()
					.name("idx_directory_paths_path")
					.table(DirectoryPaths::Table)
					.col(DirectoryPaths::Path)
					.to_owned(),
			)
			.await?;

		// Drop relative_path column from entries table
		manager
			.alter_table(
				Table::alter()
					.table(Entries::Table)
					.drop_column(Alias::new("relative_path"))
					.to_owned(),
			)
			.await?;

		// Modify locations table - drop path column
		manager
			.alter_table(
				Table::alter()
					.table(Locations::Table)
					.drop_column(Alias::new("path"))
					.to_owned(),
			)
			.await?;

		// Add entry_id column to locations table
		manager
			.alter_table(
				Table::alter()
					.table(Locations::Table)
					.add_column(
						ColumnDef::new(Locations::EntryId)
							.integer()
							.not_null(),
					)
					.to_owned(),
			)
			.await?;

		// Create foreign key for locations.entry_id
		manager
			.create_foreign_key(
				ForeignKey::create()
					.name("fk_locations_entry_id")
					.from(Locations::Table, Locations::EntryId)
					.to(Entries::Table, Entries::Id)
					.on_delete(ForeignKeyAction::Cascade)
					.on_update(ForeignKeyAction::Cascade)
					.to_owned(),
			)
			.await?;

		// Create index on locations.entry_id
		manager
			.create_index(
				Index::create()
					.name("idx_locations_entry_id")
					.table(Locations::Table)
					.col(Locations::EntryId)
					.to_owned(),
			)
			.await?;

		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Drop index on locations.entry_id
		manager
			.drop_index(
				Index::drop()
					.name("idx_locations_entry_id")
					.table(Locations::Table)
					.to_owned(),
			)
			.await?;

		// Drop foreign key for locations.entry_id
		manager
			.drop_foreign_key(
				ForeignKey::drop()
					.name("fk_locations_entry_id")
					.table(Locations::Table)
					.to_owned(),
			)
			.await?;

		// Drop entry_id column from locations table
		manager
			.alter_table(
				Table::alter()
					.table(Locations::Table)
					.drop_column(Locations::EntryId)
					.to_owned(),
			)
			.await?;

		// Add back path column to locations table
		manager
			.alter_table(
				Table::alter()
					.table(Locations::Table)
					.add_column(
						ColumnDef::new(Alias::new("path"))
							.text()
							.not_null(),
					)
					.to_owned(),
			)
			.await?;

		// Add back relative_path column to entries table
		manager
			.alter_table(
				Table::alter()
					.table(Entries::Table)
					.add_column(
						ColumnDef::new(Alias::new("relative_path"))
							.text()
							.not_null(),
					)
					.to_owned(),
			)
			.await?;

		// Drop directory_paths table
		manager
			.drop_table(Table::drop().table(DirectoryPaths::Table).to_owned())
			.await?;

		Ok(())
	}
}

#[derive(DeriveIden)]
enum Entries {
	Table,
	Id,
}

#[derive(DeriveIden)]
enum DirectoryPaths {
	Table,
	EntryId,
	Path,
}

#[derive(DeriveIden)]
enum Locations {
	Table,
	EntryId,
}