use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
	fn name(&self) -> &str {
		"m20240107_000001_create_collections"
	}
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Create collections table
		manager
			.create_table(
				Table::create()
					.table(Collection::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(Collection::Id)
							.integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(
						ColumnDef::new(Collection::Uuid)
							.uuid()
							.not_null()
							.unique_key(),
					)
					.col(ColumnDef::new(Collection::Name).string().not_null())
					.col(ColumnDef::new(Collection::Description).text().null())
					.col(
						ColumnDef::new(Collection::CreatedAt)
							.timestamp()
							.not_null()
							.default(Expr::current_timestamp()),
					)
					.col(
						ColumnDef::new(Collection::UpdatedAt)
							.timestamp()
							.not_null()
							.default(Expr::current_timestamp()),
					)
					.to_owned(),
			)
			.await?;

		// Create collection_entries junction table
		manager
			.create_table(
				Table::create()
					.table(CollectionEntry::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(CollectionEntry::CollectionId)
							.integer()
							.not_null(),
					)
					.col(
						ColumnDef::new(CollectionEntry::EntryId)
							.integer()
							.not_null(),
					)
					.col(
						ColumnDef::new(CollectionEntry::AddedAt)
							.timestamp()
							.not_null()
							.default(Expr::current_timestamp()),
					)
					.primary_key(
						Index::create()
							.col(CollectionEntry::CollectionId)
							.col(CollectionEntry::EntryId),
					)
					.foreign_key(
						ForeignKey::create()
							.name("fk_collection_entry_collection")
							.from(CollectionEntry::Table, CollectionEntry::CollectionId)
							.to(Collection::Table, Collection::Id)
							.on_delete(ForeignKeyAction::Cascade),
					)
					.foreign_key(
						ForeignKey::create()
							.name("fk_collection_entry_entry")
							.from(CollectionEntry::Table, CollectionEntry::EntryId)
							.to(Entries::Table, Entries::Id)
							.on_delete(ForeignKeyAction::Cascade),
					)
					.to_owned(),
			)
			.await?;

		// Create indexes
		manager
			.create_index(
				Index::create()
					.name("idx_collection_name")
					.table(Collection::Table)
					.col(Collection::Name)
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.name("idx_collection_entry_entry_id")
					.table(CollectionEntry::Table)
					.col(CollectionEntry::EntryId)
					.to_owned(),
			)
			.await?;

		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.drop_table(Table::drop().table(CollectionEntry::Table).to_owned())
			.await?;

		manager
			.drop_table(Table::drop().table(Collection::Table).to_owned())
			.await?;

		Ok(())
	}
}

#[derive(Iden)]
enum Collection {
	Table,
	Id,
	Uuid,
	Name,
	Description,
	CreatedAt,
	UpdatedAt,
}

#[derive(Iden)]
enum CollectionEntry {
	Table,
	CollectionId,
	EntryId,
	AddedAt,
}

#[derive(Iden)]
enum Entries {
	Table,
	Id,
}
