//! Database infrastructure using SeaORM

use sea_orm::{ConnectOptions, Database as SeaDatabase, DatabaseConnection, DbErr};
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

	/// Run migrations
	pub async fn migrate(&self) -> Result<(), DbErr> {
		migration::Migrator::up(&self.conn, None).await?;
		info!("Database migrations completed successfully");
		Ok(())
	}

	/// Get the database connection
	pub fn conn(&self) -> &DatabaseConnection {
		&self.conn
	}
}
