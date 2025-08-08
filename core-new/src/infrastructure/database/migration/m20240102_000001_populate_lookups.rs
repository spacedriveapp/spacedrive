//! Populate lookup tables with initial data

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Populate content_kinds table
        let insert_kinds = Query::insert()
            .into_table(ContentKinds::Table)
            .columns([ContentKinds::Id, ContentKinds::Name])
            .values_panic([0.into(), "unknown".into()])
            .values_panic([1.into(), "image".into()])
            .values_panic([2.into(), "video".into()])
            .values_panic([3.into(), "audio".into()])
            .values_panic([4.into(), "document".into()])
            .values_panic([5.into(), "archive".into()])
            .values_panic([6.into(), "code".into()])
            .values_panic([7.into(), "text".into()])
            .values_panic([8.into(), "database".into()])
            .values_panic([9.into(), "book".into()])
            .values_panic([10.into(), "font".into()])
            .values_panic([11.into(), "mesh".into()])
            .values_panic([12.into(), "config".into()])
            .values_panic([13.into(), "encrypted".into()])
            .values_panic([14.into(), "key".into()])
            .values_panic([15.into(), "executable".into()])
            .values_panic([16.into(), "binary".into()])
            .to_owned();

        manager.exec_stmt(insert_kinds).await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Delete all content kinds
        let delete = Query::delete()
            .from_table(ContentKinds::Table)
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