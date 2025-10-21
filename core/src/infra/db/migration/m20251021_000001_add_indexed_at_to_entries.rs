use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Add indexed_at column to entries table
		// Records when the entry was indexed/synced (not when the file was modified)
		// Used for accurate watermark-based incremental sync
		manager
			.alter_table(
				Table::alter()
					.table(Entries::Table)
					.add_column_if_not_exists(
						ColumnDef::new(Entries::IndexedAt)
							.timestamp_with_time_zone()
							.null(),
					)
					.to_owned(),
			)
			.await?;

		// Backfill indexed_at for existing entries
		// Use modified_at as fallback since we don't know actual index time
		manager
			.exec_stmt(
				Query::update()
					.table(Entries::Table)
					.value(Entries::IndexedAt, Expr::col(Entries::ModifiedAt))
					.cond_where(Expr::col(Entries::IndexedAt).is_null())
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
enum Entries {
	Table,
	IndexedAt,
	ModifiedAt,
}
