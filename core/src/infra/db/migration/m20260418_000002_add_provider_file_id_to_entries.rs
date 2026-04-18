//! Add nullable `provider_file_id` column on `entries` plus a partial index
//! keyed by `(volume_id, provider_file_id)`.
//!
//! Rename and move tracking on OneDrive / Google Drive / Dropbox depends on
//! following entries by their stable provider id, because path can change
//! across a delta page while the id does not. Storing the id on the entry row
//! lets `PathResolver::resolve_to_entry` look up a cloud entry by id in O(log
//! N) and fall back to path-only when the id is missing (local or
//! non-delta-capable volumes). The index is partial so non-cloud entries pay
//! no space overhead.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.alter_table(
				Table::alter()
					.table(Entries::Table)
					.add_column(ColumnDef::new(Entries::ProviderFileId).string().null())
					.to_owned(),
			)
			.await?;

		// Partial index. SeaORM's schema builder does not expose SQLite
		// partial indexes, so we emit raw SQL. Scoping by `volume_id` keeps
		// lookups unique within a single cloud volume, which matches how the
		// indexer resolves paths.
		let db = manager.get_connection();
		let backend = db.get_database_backend();
		db.execute(sea_orm::Statement::from_string(
			backend,
			"CREATE INDEX IF NOT EXISTS idx_entries_provider_file_id \
			 ON entries (volume_id, provider_file_id) \
			 WHERE provider_file_id IS NOT NULL"
				.to_string(),
		))
		.await?;

		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		let db = manager.get_connection();
		let backend = db.get_database_backend();
		db.execute(sea_orm::Statement::from_string(
			backend,
			"DROP INDEX IF EXISTS idx_entries_provider_file_id".to_string(),
		))
		.await?;

		manager
			.alter_table(
				Table::alter()
					.table(Entries::Table)
					.drop_column(Entries::ProviderFileId)
					.to_owned(),
			)
			.await?;

		Ok(())
	}
}

#[derive(DeriveIden)]
enum Entries {
	Table,
	ProviderFileId,
}
