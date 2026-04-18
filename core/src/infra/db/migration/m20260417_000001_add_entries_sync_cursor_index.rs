//! Add composite index on entries(indexed_at, uuid) to back the device-owned
//! sync cursor.
//!
//! `Entry::query_for_sync` paginates by `ORDER BY indexed_at ASC, uuid ASC`
//! with a tie-breaker filter of the same shape. Without this index, SQLite
//! does a full table scan per batch request — O(N) per batch, O(N^2) across
//! an initial backfill of a large library.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.get_connection()
			.execute_unprepared(
				"CREATE INDEX IF NOT EXISTS idx_entries_indexed_at_uuid \
				 ON entries(indexed_at, uuid)",
			)
			.await?;

		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.get_connection()
			.execute_unprepared("DROP INDEX IF EXISTS idx_entries_indexed_at_uuid")
			.await?;

		Ok(())
	}
}
