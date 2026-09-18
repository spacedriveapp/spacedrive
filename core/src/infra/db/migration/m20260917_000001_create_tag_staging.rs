//! A tag created and applied to nothing has no source store to live in, so
//! the library holds it in a small staging table until a source adopts it on
//! first application. This is the one denormalization the tag model accepts:
//! the alternative is that creating a tag is silently a no-op until it is
//! used. Columns mirror `tag_definition` in the source store schema.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.create_table(
				Table::create()
					.table(TagStaging::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(TagStaging::Id)
							.integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(
						ColumnDef::new(TagStaging::Uuid)
							.uuid()
							.not_null()
							.unique_key(),
					)
					.col(ColumnDef::new(TagStaging::SlugId).uuid().not_null())
					.col(ColumnDef::new(TagStaging::Path).string().not_null())
					.col(ColumnDef::new(TagStaging::Color).string())
					.col(ColumnDef::new(TagStaging::Icon).string())
					.col(ColumnDef::new(TagStaging::UpdatedHlc).string().not_null())
					.col(ColumnDef::new(TagStaging::OriginDevice).uuid().not_null())
					.to_owned(),
			)
			.await
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.drop_table(Table::drop().table(TagStaging::Table).to_owned())
			.await
	}
}

#[derive(DeriveIden)]
enum TagStaging {
	Table,
	Id,
	Uuid,
	SlugId,
	Path,
	Color,
	Icon,
	UpdatedHlc,
	OriginDevice,
}
