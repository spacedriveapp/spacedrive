use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Add version column to audit_log table for sync support
		manager
			.alter_table(
				Table::alter()
					.table(AuditLog::Table)
					.add_column_if_not_exists(
						ColumnDef::new(AuditLog::Version)
							.big_integer()
							.not_null()
							.default(1),
					)
					.to_owned(),
			)
			.await?;

		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// SQLite doesn't support DROP COLUMN easily
		// Would require table recreation
		Ok(())
	}
}

#[derive(DeriveIden)]
enum AuditLog {
	Table,
	Version,
}
