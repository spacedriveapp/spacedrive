//! Add audit log table for action tracking

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create the table first
        manager
            .create_table(
                Table::create()
                    .table(AuditLog::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AuditLog::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(AuditLog::Uuid).text().not_null().unique_key())
                    .col(ColumnDef::new(AuditLog::ActionType).string().not_null())
                    .col(ColumnDef::new(AuditLog::ActorDeviceId).text().not_null())
                    .col(ColumnDef::new(AuditLog::Targets).text().not_null())
                    .col(ColumnDef::new(AuditLog::Status).string().not_null())
                    .col(ColumnDef::new(AuditLog::JobId).text())
                    .col(
                        ColumnDef::new(AuditLog::CreatedAt)
                            .timestamp()
                            .not_null(),
                    )
                    .col(ColumnDef::new(AuditLog::CompletedAt).timestamp())
                    .col(ColumnDef::new(AuditLog::ErrorMessage).text())
                    .col(ColumnDef::new(AuditLog::ResultPayload).text())
                    .to_owned(),
            )
            .await?;

        // Create indexes separately
        manager
            .create_index(
                Index::create()
                    .name("idx_audit_log_action_type")
                    .table(AuditLog::Table)
                    .col(AuditLog::ActionType)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_audit_log_actor_device_id")
                    .table(AuditLog::Table)
                    .col(AuditLog::ActorDeviceId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_audit_log_status")
                    .table(AuditLog::Table)
                    .col(AuditLog::Status)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_audit_log_job_id")
                    .table(AuditLog::Table)
                    .col(AuditLog::JobId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_audit_log_created_at")
                    .table(AuditLog::Table)
                    .col(AuditLog::CreatedAt)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(AuditLog::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum AuditLog {
    Table,
    Id,
    Uuid,
    ActionType,
    ActorDeviceId,
    Targets,
    Status,
    JobId,
    CreatedAt,
    CompletedAt,
    ErrorMessage,
    ResultPayload,
}