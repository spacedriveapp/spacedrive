use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create sidecars table
        manager
            .create_table(
                Table::create()
                    .table(Sidecar::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Sidecar::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Sidecar::ContentUuid)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Sidecar::Kind)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Sidecar::Variant)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Sidecar::Format)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Sidecar::RelPath)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Sidecar::Size)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Sidecar::Checksum)
                            .string()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(Sidecar::Status)
                            .string()
                            .not_null()
                            .default("pending"),
                    )
                    .col(
                        ColumnDef::new(Sidecar::Source)
                            .string()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(Sidecar::Version)
                            .integer()
                            .not_null()
                            .default(1),
                    )
                    .col(
                        ColumnDef::new(Sidecar::CreatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Sidecar::UpdatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_sidecar_content")
                            .from(Sidecar::Table, Sidecar::ContentUuid)
                            .to(ContentIdentities::Table, ContentIdentities::Uuid)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create unique index on (content_uuid, kind, variant)
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_sidecar_unique")
                    .table(Sidecar::Table)
                    .col(Sidecar::ContentUuid)
                    .col(Sidecar::Kind)
                    .col(Sidecar::Variant)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Create sidecar_availability table
        manager
            .create_table(
                Table::create()
                    .table(SidecarAvailability::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SidecarAvailability::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(SidecarAvailability::ContentUuid)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SidecarAvailability::Kind)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SidecarAvailability::Variant)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SidecarAvailability::DeviceUuid)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SidecarAvailability::Has)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(SidecarAvailability::Size)
                            .big_integer()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(SidecarAvailability::Checksum)
                            .string()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(SidecarAvailability::LastSeenAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_sidecar_availability_content")
                            .from(SidecarAvailability::Table, SidecarAvailability::ContentUuid)
                            .to(ContentIdentities::Table, ContentIdentities::Uuid)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_sidecar_availability_device")
                            .from(SidecarAvailability::Table, SidecarAvailability::DeviceUuid)
                            .to(Devices::Table, Devices::Uuid)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create unique index on (content_uuid, kind, variant, device_uuid)
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_sidecar_availability_unique")
                    .table(SidecarAvailability::Table)
                    .col(SidecarAvailability::ContentUuid)
                    .col(SidecarAvailability::Kind)
                    .col(SidecarAvailability::Variant)
                    .col(SidecarAvailability::DeviceUuid)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop sidecar_availability table
        manager
            .drop_table(Table::drop().table(SidecarAvailability::Table).to_owned())
            .await?;

        // Drop sidecars table
        manager
            .drop_table(Table::drop().table(Sidecar::Table).to_owned())
            .await?;

        Ok(())
    }
}

#[derive(Iden)]
enum Sidecar {
    Table,
    Id,
    ContentUuid,
    Kind,
    Variant,
    Format,
    RelPath,
    Size,
    Checksum,
    Status,
    Source,
    Version,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum SidecarAvailability {
    Table,
    Id,
    ContentUuid,
    Kind,
    Variant,
    DeviceUuid,
    Has,
    Size,
    Checksum,
    LastSeenAt,
}

#[derive(Iden)]
enum ContentIdentities {
    Table,
    Uuid,
}

#[derive(Iden)]
enum Devices {
    Table,
    Uuid,
}