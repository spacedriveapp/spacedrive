//! FTS5 Search Index Migration
//!
//! Creates FTS5 virtual table for high-performance full-text search
//! and associated triggers for real-time index updates.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Create FTS5 virtual table for search indexing
		manager
			.get_connection()
			.execute_unprepared(
				r#"
                CREATE VIRTUAL TABLE search_index USING fts5(
                    content='entries',
                    content_rowid='id',
                    name,
                    extension,
                    tokenize="unicode61 remove_diacritics 2 tokenchars '.@-_'",
                    prefix='2,3'
                );
                "#,
			)
			.await?;

		// Create trigger for INSERT operations
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

		// Create trigger for UPDATE operations
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

		// Create trigger for DELETE operations
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

		// Populate FTS5 index with existing file entries
		manager
			.get_connection()
			.execute_unprepared(
				r#"
                INSERT INTO search_index(rowid, name, extension)
                SELECT id, name, extension FROM entries WHERE kind = 0;
                "#,
			)
			.await?;

		// Create search analytics table for query optimization
		manager
			.get_connection()
			.execute_unprepared(
				r#"
                CREATE TABLE search_analytics (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    query_text TEXT NOT NULL,
                    query_hash TEXT NOT NULL,
                    search_mode TEXT NOT NULL,
                    execution_time_ms INTEGER NOT NULL,
                    result_count INTEGER NOT NULL,
                    fts5_used BOOLEAN DEFAULT TRUE,
                    semantic_used BOOLEAN DEFAULT FALSE,
                    user_clicked_result BOOLEAN DEFAULT FALSE,
                    clicked_result_position INTEGER,
                    created_at TEXT NOT NULL DEFAULT (datetime('now'))
                );
                "#,
			)
			.await?;

		// Create index on query_hash for performance analytics
		manager
			.get_connection()
			.execute_unprepared(
				r#"
                CREATE INDEX idx_search_analytics_query_hash
                ON search_analytics(query_hash);
                "#,
			)
			.await?;

		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Drop analytics table and index
		manager
			.get_connection()
			.execute_unprepared("DROP INDEX IF EXISTS idx_search_analytics_query_hash;")
			.await?;

		manager
			.get_connection()
			.execute_unprepared("DROP TABLE IF EXISTS search_analytics;")
			.await?;

		// Drop triggers
		manager
			.get_connection()
			.execute_unprepared("DROP TRIGGER IF EXISTS entries_search_delete;")
			.await?;

		manager
			.get_connection()
			.execute_unprepared("DROP TRIGGER IF EXISTS entries_search_update;")
			.await?;

		manager
			.get_connection()
			.execute_unprepared("DROP TRIGGER IF EXISTS entries_search_insert;")
			.await?;

		// Drop FTS5 virtual table
		manager
			.get_connection()
			.execute_unprepared("DROP TABLE IF EXISTS search_index;")
			.await?;

		Ok(())
	}
}
