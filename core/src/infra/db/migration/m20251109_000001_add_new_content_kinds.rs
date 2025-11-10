//! Add new content kinds (Spreadsheet, Presentation, Email, etc.)

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Add new content kinds that were added to ContentKind enum
		let insert_kinds = Query::insert()
			.into_table(ContentKinds::Table)
			.columns([ContentKinds::Id, ContentKinds::Name])
			.values_panic([17.into(), "spreadsheet".into()])
			.values_panic([18.into(), "presentation".into()])
			.values_panic([19.into(), "email".into()])
			.values_panic([20.into(), "calendar".into()])
			.values_panic([21.into(), "contact".into()])
			.values_panic([22.into(), "web".into()])
			.values_panic([23.into(), "shortcut".into()])
			.values_panic([24.into(), "package".into()])
			.to_owned();

		manager.exec_stmt(insert_kinds).await?;

		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Delete the new content kinds
		let delete = Query::delete()
			.from_table(ContentKinds::Table)
			.and_where(Expr::col(ContentKinds::Id).gte(17))
			.to_owned();
		manager.exec_stmt(delete).await?;

		Ok(())
	}
}

#[derive(DeriveIden)]
enum ContentKinds {
	Table,
	Id,
	Name,
}
