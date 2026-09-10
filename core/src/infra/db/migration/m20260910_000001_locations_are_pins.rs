//! A location stops owning records and becomes a pin.
//!
//! The old shape rooted a location at an entry and carried its scan state and
//! totals. Entries are gone, the arena keeps the totals, and the source keeps
//! the scan state, so what is left is a name, a path relative to the source
//! root, and whether someone put it there.
//!
//! No rows carry over. Every one of them was anchored to an `entry_id` in a
//! table this migration series removes, so preserving them would preserve
//! exactly the link being taken out. The defaults are written fresh when a
//! library opens.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.drop_table(Table::drop().table(Locations::Table).if_exists().to_owned())
			.await?;

		manager
			.create_table(
				Table::create()
					.table(Location::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(Location::Id)
							.integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(
						ColumnDef::new(Location::Uuid)
							.uuid()
							.not_null()
							.unique_key(),
					)
					.col(ColumnDef::new(Location::SourceUuid).uuid().not_null())
					.col(
						ColumnDef::new(Location::RelativePath)
							.string()
							.not_null()
							.default(""),
					)
					.col(ColumnDef::new(Location::Name).string().not_null())
					.col(
						ColumnDef::new(Location::Origin)
							.string()
							.not_null()
							.default("user"),
					)
					.col(
						ColumnDef::new(Location::CreatedAt)
							.timestamp_with_time_zone()
							.not_null(),
					)
					.to_owned(),
			)
			.await?;

		// One pin per place. Pinning the same folder twice is the same pin.
		manager
			.create_index(
				Index::create()
					.name("idx_location_source_path")
					.table(Location::Table)
					.col(Location::SourceUuid)
					.col(Location::RelativePath)
					.unique()
					.to_owned(),
			)
			.await
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.drop_table(Table::drop().table(Location::Table).to_owned())
			.await
	}
}

/// The table as it was, named plurally.
#[derive(DeriveIden)]
enum Locations {
	Table,
}

#[derive(DeriveIden)]
enum Location {
	Table,
	Id,
	Uuid,
	SourceUuid,
	RelativePath,
	Name,
	Origin,
	CreatedAt,
}
