use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Add unique index on entries (parent_id, name, extension) to prevent duplicate files
		// This prevents race conditions in the watcher where multiple Create events
		// for the same file (e.g., rapidly recreated temp files) can create duplicates
		//
		// Note: We only apply this to files (kind = 0) with a parent directory
		// Directories, symlinks, and root entries are excluded
		manager
			.get_connection()
			.execute_unprepared(
				r#"
				CREATE UNIQUE INDEX IF NOT EXISTS idx_entries_unique_file
				ON entries(parent_id, name, extension)
				WHERE kind = 0 AND parent_id IS NOT NULL
				"#,
			)
			.await?;

		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Drop the unique index
		manager
			.drop_index(
				Index::drop()
					.name("idx_entries_unique_file")
					.table(Entries::Table)
					.to_owned(),
			)
			.await?;

		Ok(())
	}
}

#[derive(DeriveIden)]
enum Entries {
	Table,
}



