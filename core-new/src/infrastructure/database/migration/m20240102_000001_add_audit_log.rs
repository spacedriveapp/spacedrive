//! Add audit log table for action tracking

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
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
                    .col(ColumnDef::new(AuditLog::Uuid).uuid().not_null().unique_key())
                    .col(ColumnDef::new(AuditLog::ActionType).string().not_null())
                    .col(ColumnDef::new(AuditLog::ActorDeviceId).uuid().not_null())
                    .col(ColumnDef::new(AuditLog::Targets).json().not_null())
                    .col(ColumnDef::new(AuditLog::Status).string().not_null())
                    .col(ColumnDef::new(AuditLog::JobId).uuid())
                    .col(
                        ColumnDef::new(AuditLog::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(AuditLog::CompletedAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(AuditLog::ErrorMessage).text())
                    .col(ColumnDef::new(AuditLog::ResultPayload).json())
                    .index(
                        Index::create()
                            .name("idx_audit_log_action_type")
                            .col(AuditLog::ActionType),
                    )
                    .index(
                        Index::create()
                            .name("idx_audit_log_actor_device_id")
                            .col(AuditLog::ActorDeviceId),
                    )
                    .index(
                        Index::create()
                            .name("idx_audit_log_status")
                            .col(AuditLog::Status),
                    )
                    .index(
                        Index::create()
                            .name("idx_audit_log_job_id")
                            .col(AuditLog::JobId),
                    )
                    .index(
                        Index::create()
                            .name("idx_audit_log_created_at")
                            .col(AuditLog::CreatedAt),
                    )
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