use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(IndexerRules::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(IndexerRules::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(IndexerRules::Name).string().not_null().unique_key())
                    .col(ColumnDef::new(IndexerRules::Default).boolean().not_null())
                    .col(ColumnDef::new(IndexerRules::RulesBlob).binary().not_null())
                    .col(ColumnDef::new(IndexerRules::CreatedAt).timestamp_with_time_zone().not_null())
                    .col(ColumnDef::new(IndexerRules::UpdatedAt).timestamp_with_time_zone().not_null())
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(IndexerRules::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum IndexerRules {
    Table,
    Id,
    Name,
    Default,
    RulesBlob,
    CreatedAt,
    UpdatedAt,
}



