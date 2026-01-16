//! Fix FTS5 Search Index to Include Directories
//!
//! The original search index migration filtered out directories (kind = 1)
//! in triggers and initial population. This migration fixes that by:
//! 1. Dropping and recreating triggers without kind filters
//! 2. Adding existing directories to the search index

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Drop old triggers that filtered out directories
		manager
			.get_connection()
			.execute_unprepared("DROP TRIGGER IF EXISTS entries_search_insert;")
			.await?;

		manager
			.get_connection()
			.execute_unprepared("DROP TRIGGER IF EXISTS entries_search_update;")
			.await?;

		manager
			.get_connection()
			.execute_unprepared("DROP TRIGGER IF EXISTS entries_search_delete;")
			.await?;

		// Recreate INSERT trigger without kind filter
		manager
			.get_connection()
			.execute_unprepared(
				r#"
                CREATE TRIGGER IF NOT EXISTS entries_search_insert
                AFTER INSERT ON entries
                BEGIN
                    INSERT INTO search_index(rowid, name, extension)
                    VALUES (new.id, new.name, new.extension);
                END;
                "#,
			)
			.await?;

		// Recreate UPDATE trigger without kind filter
		manager
			.get_connection()
			.execute_unprepared(
				r#"
                CREATE TRIGGER IF NOT EXISTS entries_search_update
                AFTER UPDATE ON entries
                BEGIN
                    UPDATE search_index SET
                        name = new.name,
                        extension = new.extension
                    WHERE rowid = new.id;
                END;
                "#,
			)
			.await?;

		// Recreate DELETE trigger without kind filter
		manager
			.get_connection()
			.execute_unprepared(
				r#"
                CREATE TRIGGER IF NOT EXISTS entries_search_delete
                AFTER DELETE ON entries
                BEGIN
                    DELETE FROM search_index WHERE rowid = old.id;
                END;
                "#,
			)
			.await?;

		// Add existing directories to the search index
		// Use INSERT OR IGNORE in case some directories were already indexed
		manager
			.get_connection()
			.execute_unprepared(
				r#"
                INSERT OR IGNORE INTO search_index(rowid, name, extension)
                SELECT id, name, extension FROM entries WHERE kind = 1;
                "#,
			)
			.await?;

		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Revert to old triggers that filter out directories
		manager
			.get_connection()
			.execute_unprepared("DROP TRIGGER IF EXISTS entries_search_insert;")
			.await?;

		manager
			.get_connection()
			.execute_unprepared("DROP TRIGGER IF EXISTS entries_search_update;")
			.await?;

		manager
			.get_connection()
			.execute_unprepared("DROP TRIGGER IF EXISTS entries_search_delete;")
			.await?;

		// Recreate old INSERT trigger with kind filter
		manager
			.get_connection()
			.execute_unprepared(
				r#"
                CREATE TRIGGER IF NOT EXISTS entries_search_insert
                AFTER INSERT ON entries WHEN new.kind = 0
                BEGIN
                    INSERT INTO search_index(rowid, name, extension)
                    VALUES (new.id, new.name, new.extension);
                END;
                "#,
			)
			.await?;

		// Recreate old UPDATE trigger with kind filter
		manager
			.get_connection()
			.execute_unprepared(
				r#"
                CREATE TRIGGER IF NOT EXISTS entries_search_update
                AFTER UPDATE ON entries WHEN new.kind = 0
                BEGIN
                    UPDATE search_index SET
                        name = new.name,
                        extension = new.extension
                    WHERE rowid = new.id;
                END;
                "#,
			)
			.await?;

		// Recreate old DELETE trigger with kind filter
		manager
			.get_connection()
			.execute_unprepared(
				r#"
                CREATE TRIGGER IF NOT EXISTS entries_search_delete
                AFTER DELETE ON entries WHEN old.kind = 0
                BEGIN
                    DELETE FROM search_index WHERE rowid = old.id;
                END;
                "#,
			)
			.await?;

		// Remove directories from the search index
		manager
			.get_connection()
			.execute_unprepared(
				r#"
                DELETE FROM search_index
                WHERE rowid IN (SELECT id FROM entries WHERE kind = 1);
                "#,
			)
			.await?;

		Ok(())
	}
}
