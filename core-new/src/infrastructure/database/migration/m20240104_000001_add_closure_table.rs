//! Add closure table for efficient hierarchical queries

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Add parent_id column to entries table
		manager
			.alter_table(
				Table::alter()
					.table(Entries::Table)
					.add_column(
						ColumnDef::new(Entries::ParentId)
							.integer()
							.null()
					)
					.to_owned(),
			)
			.await?;

		// Create foreign key for parent_id
		manager
			.create_foreign_key(
				ForeignKey::create()
					.name("fk_entries_parent_id")
					.from(Entries::Table, Entries::ParentId)
					.to(Entries::Table, Entries::Id)
					.on_delete(ForeignKeyAction::SetNull)
					.on_update(ForeignKeyAction::Cascade)
					.to_owned(),
			)
			.await?;

		// Create entry_closure table
		manager
			.create_table(
				Table::create()
					.table(EntryClosure::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(EntryClosure::AncestorId)
							.integer()
							.not_null(),
					)
					.col(
						ColumnDef::new(EntryClosure::DescendantId)
							.integer()
							.not_null(),
					)
					.col(
						ColumnDef::new(EntryClosure::Depth)
							.integer()
							.not_null(),
					)
					.primary_key(
						Index::create()
							.col(EntryClosure::AncestorId)
							.col(EntryClosure::DescendantId),
					)
					.foreign_key(
						ForeignKey::create()
							.name("fk_closure_ancestor")
							.from(EntryClosure::Table, EntryClosure::AncestorId)
							.to(Entries::Table, Entries::Id)
							.on_delete(ForeignKeyAction::Cascade)
							.on_update(ForeignKeyAction::Cascade),
					)
					.foreign_key(
						ForeignKey::create()
							.name("fk_closure_descendant")
							.from(EntryClosure::Table, EntryClosure::DescendantId)
							.to(Entries::Table, Entries::Id)
							.on_delete(ForeignKeyAction::Cascade)
							.on_update(ForeignKeyAction::Cascade),
					)
					.to_owned(),
			)
			.await?;

		// Create index on descendant_id for efficient ancestor lookups
		manager
			.create_index(
				Index::create()
					.name("idx_closure_descendant")
					.table(EntryClosure::Table)
					.col(EntryClosure::DescendantId)
					.to_owned(),
			)
			.await?;

		// Create compound index on ancestor_id and depth for efficient child/descendant queries
		manager
			.create_index(
				Index::create()
					.name("idx_closure_ancestor_depth")
					.table(EntryClosure::Table)
					.col(EntryClosure::AncestorId)
					.col(EntryClosure::Depth)
					.to_owned(),
			)
			.await?;

		// Create index on parent_id for efficient child lookups
		manager
			.create_index(
				Index::create()
					.name("idx_entries_parent_id")
					.table(Entries::Table)
					.col(Entries::ParentId)
					.to_owned(),
			)
			.await?;

		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Drop entry_closure table (this will also drop all its indexes and foreign keys)
		manager
			.drop_table(Table::drop().table(EntryClosure::Table).to_owned())
			.await?;

		// Drop the parent_id foreign key constraint
		manager
			.drop_foreign_key(
				ForeignKey::drop()
					.name("fk_entries_parent_id")
					.table(Entries::Table)
					.to_owned(),
			)
			.await?;

		// Drop the parent_id index
		manager
			.drop_index(
				Index::drop()
					.name("idx_entries_parent_id")
					.table(Entries::Table)
					.to_owned(),
			)
			.await?;

		// Remove parent_id column from entries table
		manager
			.alter_table(
				Table::alter()
					.table(Entries::Table)
					.drop_column(Entries::ParentId)
					.to_owned(),
			)
			.await?;

		Ok(())
	}
}

#[derive(DeriveIden)]
enum Entries {
	Table,
	Id,
	ParentId,
}

#[derive(DeriveIden)]
enum EntryClosure {
	Table,
	AncestorId,
	DescendantId,
	Depth,
}