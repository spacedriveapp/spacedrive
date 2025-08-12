//! Test database migration functionality

use sd_core_new::infrastructure::database::{migration::Migrator, Database};
use tempfile::TempDir;

#[tokio::test]
async fn test_database_creation_and_migration() {
	// Create a temporary directory for the test database
	let temp_dir = TempDir::new().unwrap();
	let db_path = temp_dir.path().join("test.db");

	println!("Creating database at: {:?}", db_path);

	// Create the database
	let db = Database::create(&db_path)
		.await
		.expect("Failed to create database");

	println!("Database created successfully, running migrations...");

	// Run migrations with debug info
	println!("Running migrations...");
	let result = db.migrate().await;

	match result {
		Ok(()) => {
			println!("✅ Migrations completed successfully!");
		}
		Err(e) => {
			println!("❌ Migration failed: {}", e);
			panic!("Migration failed: {}", e);
		}
	}

	// Verify the database exists and has tables
	assert!(db_path.exists(), "Database file should exist");

	// Try to connect to verify it's a valid database
	let conn = db.conn();

	// Try a simple query to verify the database is working
	use sea_orm::{ConnectionTrait, Statement};

	let result = conn
		.execute(Statement::from_string(
			sea_orm::DatabaseBackend::Sqlite,
			"SELECT name FROM sqlite_master WHERE type='table' ORDER BY name;".to_string(),
		))
		.await;

	match result {
		Ok(result) => {
			println!(
				"✅ Database query successful, {} rows affected",
				result.rows_affected()
			);
		}
		Err(e) => {
			println!("❌ Database query failed: {}", e);
			panic!("Database query failed: {}", e);
		}
	}
}
