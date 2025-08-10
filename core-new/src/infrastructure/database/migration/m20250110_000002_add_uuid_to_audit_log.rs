use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add uuid column to audit_log table
        // SQLite doesn't support adding unique columns without default value
        manager
            .alter_table(
                Table::alter()
                    .table(AuditLog::Table)
                    .add_column(
                        ColumnDef::new(AuditLog::Uuid)
                            .string()  // SQLite doesn't have native UUID type
                            .not_null()
                            .default("")  // Temporary default, will be populated later
                    )
                    .to_owned(),
            )
            .await?;

        // Create index on uuid for performance
        manager
            .create_index(
                Index::create()
                    .name("idx_audit_log_uuid")
                    .table(AuditLog::Table)
                    .col(AuditLog::Uuid)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop the index first
        manager
            .drop_index(
                Index::drop()
                    .name("idx_audit_log_uuid")
                    .table(AuditLog::Table)
                    .to_owned(),
            )
            .await?;

        // Remove the uuid column
        manager
            .alter_table(
                Table::alter()
                    .table(AuditLog::Table)
                    .drop_column(AuditLog::Uuid)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum AuditLog {
    Table,
    Uuid,
}