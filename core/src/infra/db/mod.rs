//! Database infrastructure using SeaORM

use sea_orm::{
	ConnectOptions, Database as SeaDatabase, DatabaseConnection, DbErr, TransactionTrait,
};
use sea_orm_migration::MigratorTrait;
use sqlx::sqlite::SqliteConnectOptions;
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;
use tracing::info;

pub mod entities;
pub mod migration;

/// Database wrapper for Spacedrive
pub struct Database {
	/// SeaORM database connection
	conn: DatabaseConnection,
}

impl AsRef<DatabaseConnection> for Database {
	fn as_ref(&self) -> &DatabaseConnection {
		&self.conn
	}
}

/// Build `SqliteConnectOptions` with PRAGMAs applied to every pooled connection.
fn sqlite_connect_options(url: &str) -> Result<SqliteConnectOptions, DbErr> {
	let opts = SqliteConnectOptions::from_str(url)
		.map_err(|e| DbErr::Custom(format!("Invalid SQLite URL: {}", e)))?
		.busy_timeout(Duration::from_millis(5000))
		.journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
		.synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
		.pragma("temp_store", "MEMORY")
		.pragma("cache_size", "-20000")
		.pragma("mmap_size", "67108864");
	Ok(opts)
}

/// Build a SeaORM `DatabaseConnection` from sqlx `SqliteConnectOptions`,
/// ensuring all PRAGMAs are applied to every connection in the pool.
async fn connect_sqlite(url: &str, pool_size: u32) -> Result<DatabaseConnection, DbErr> {
	let opts = sqlite_connect_options(url)?;

	let pool_size = pool_size.max(1);
	let pool = sqlx::pool::PoolOptions::<sqlx::Sqlite>::new()
		.max_connections(pool_size)
		.min_connections(pool_size.min(5))
		.acquire_timeout(Duration::from_secs(30))
		.idle_timeout(Duration::from_secs(30))
		.max_lifetime(Duration::from_secs(30))
		.connect_with(opts)
		.await
		.map_err(|e| DbErr::Custom(format!("Failed to connect: {}", e)))?;

	Ok(sea_orm::SqlxSqliteConnector::from_sqlx_sqlite_pool(pool))
}

impl Database {
	/// Create a new database at the specified path
	pub async fn create(path: &Path) -> Result<Self, DbErr> {
		let db_url = if path.as_os_str() == ":memory:" {
			"sqlite::memory:".to_string()
		} else {
			// Ensure parent directory exists
			if let Some(parent) = path.parent() {
				std::fs::create_dir_all(parent)
					.map_err(|e| DbErr::Custom(format!("Failed to create directory: {}", e)))?;
			}
			format!("sqlite://{}?mode=rwc", path.display())
		};

		let pool_size = std::env::var("SPACEDRIVE_DB_POOL_SIZE")
			.ok()
			.and_then(|s| s.parse().ok())
			.unwrap_or(30);

		let conn = connect_sqlite(&db_url, pool_size).await?;

		info!("Created new database at {:?}", path);

		Ok(Self { conn })
	}

	/// Open an existing database
	pub async fn open(path: &Path) -> Result<Self, DbErr> {
		if !path.exists() {
			return Err(DbErr::Custom(format!(
				"Database does not exist: {}",
				path.display()
			)));
		}

		let db_url = format!("sqlite://{}", path.display());

		let pool_size = std::env::var("SPACEDRIVE_DB_POOL_SIZE")
			.ok()
			.and_then(|s| s.parse().ok())
			.unwrap_or(30);

		let conn = connect_sqlite(&db_url, pool_size).await?;

		info!("Opened database at {:?}", path);

		Ok(Self { conn })
	}

	/// Run migrations on one pinned connection.
	///
	/// sea-orm-migration only wraps the run in a transaction for Postgres. On
	/// SQLite each statement would otherwise go to the pool, and the pool hands
	/// statements round-robin to connections that each cache their own copy of
	/// the schema. Most statements recover from a stale cache (SQLite re-prepares
	/// on SQLITE_SCHEMA), but ALTER TABLE DROP COLUMN resolves the column at
	/// prepare time against the cache and fails with "no such column" for good.
	/// A transaction holds a single connection for the whole run, so every
	/// statement sees the one it came after. Everything lands or nothing does.
	pub async fn migrate(&self) -> Result<(), DbErr> {
		let txn = self.conn.begin().await?;
		migration::Migrator::up(&txn, None).await?;
		txn.commit().await?;
		info!("Database migrations completed successfully");
		Ok(())
	}

	/// Get the database connection
	pub fn conn(&self) -> &DatabaseConnection {
		&self.conn
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	/// Every connection of a 30-connection pool opens before the first
	/// statement. The migrator rotates through them, and the connection that
	/// loaded the schema just before `m20251226` adds `entries.device_id` is
	/// the one handed `m20260104`'s `DROP COLUMN device_id` fourteen statements
	/// later, with a cache that never had the column. Running the migrations on
	/// the pool failed this every time; pinned to one transaction it passes.
	#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
	async fn migrate_fresh_database_on_eagerly_filled_pool() {
		let dir = tempfile::tempdir().unwrap();
		let url = format!(
			"sqlite://{}?mode=rwc",
			dir.path().join("library.db").display()
		);
		let pool = sqlx::pool::PoolOptions::<sqlx::Sqlite>::new()
			.max_connections(30)
			.min_connections(30)
			.connect_with(sqlite_connect_options(&url).unwrap())
			.await
			.unwrap();
		let db = Database {
			conn: sea_orm::SqlxSqliteConnector::from_sqlx_sqlite_pool(pool),
		};

		db.migrate()
			.await
			.expect("migrations on a stale pooled connection");
	}
}
