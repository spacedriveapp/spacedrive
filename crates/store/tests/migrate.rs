//! The store schema version against real files: who writes it, who reads it,
//! and that a `VACUUM INTO` copy keeps it.

use std::path::Path;

use sd_store::{filesystem_schema, migrate, SourceManager, SCHEMA_VERSION};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

struct Fixture {
	dir: tempfile::TempDir,
	manager: SourceManager,
}

impl Fixture {
	async fn new() -> Self {
		let dir = tempfile::tempdir().expect("tempdir");
		let manager = SourceManager::new(dir.path().join("sources"));
		manager
			.create("source-1", &filesystem_schema())
			.await
			.expect("source created");
		Self { dir, manager }
	}

	fn db_path(&self) -> std::path::PathBuf {
		self.dir.path().join("sources/source-1/data.db")
	}
}

/// A raw writable connection to a store file, outside the manager, for
/// setting up shapes the manager would never produce.
async fn raw(path: &Path) -> SqlitePool {
	SqlitePoolOptions::new()
		.max_connections(1)
		.connect_with(
			SqliteConnectOptions::new()
				.filename(path)
				.create_if_missing(true),
		)
		.await
		.expect("raw pool")
}

#[tokio::test]
async fn a_fresh_store_carries_the_current_version() {
	let fixture = Fixture::new().await;
	let pool = raw(&fixture.db_path()).await;
	assert_eq!(
		migrate::version(&pool).await.expect("version"),
		SCHEMA_VERSION
	);

	let db = fixture.manager.open("source-1").await.expect("open");
	assert_eq!(db.schema_version(), SCHEMA_VERSION);
	let reader = fixture
		.manager
		.open_read_only("source-1")
		.await
		.expect("open read-only");
	assert_eq!(reader.schema_version(), SCHEMA_VERSION);
}

#[tokio::test]
async fn vacuum_into_keeps_the_owners_version() {
	let fixture = Fixture::new().await;
	let pool = raw(&fixture.db_path()).await;
	// A value no build writes, so the copy cannot have arrived at it on its own.
	sqlx::query("PRAGMA user_version = 7")
		.execute(&pool)
		.await
		.expect("set version");

	let copy = fixture.dir.path().join("replica.db");
	sqlx::query("VACUUM INTO ?")
		.bind(copy.to_string_lossy().to_string())
		.execute(&pool)
		.await
		.expect("vacuum into");

	let replica = raw(&copy).await;
	assert_eq!(migrate::version(&replica).await.expect("version"), 7);
}

#[tokio::test]
async fn a_store_from_a_newer_build_is_refused() {
	let fixture = Fixture::new().await;
	let pool = raw(&fixture.db_path()).await;
	sqlx::query(&format!("PRAGMA user_version = {}", SCHEMA_VERSION + 1))
		.execute(&pool)
		.await
		.expect("set version");
	pool.close().await;

	let writer = fixture.manager.open("source-1").await;
	assert!(
		matches!(writer, Err(sd_store::Error::UnsupportedGeneration(_))),
		"a writer must not downgrade a store"
	);
	let reader = SourceManager::open_file_read_only(&fixture.db_path()).await;
	assert!(
		matches!(reader, Err(sd_store::Error::UnsupportedGeneration(_))),
		"a reader cannot know a shape newer than its own"
	);
}
