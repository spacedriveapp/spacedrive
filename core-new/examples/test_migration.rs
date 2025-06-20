//! Simple migration test to verify the schema works

use sd_core_new::infrastructure::database::migration::Migrator;
use sea_orm::{Database, ConnectionTrait, Statement};
use sea_orm_migration::MigratorTrait;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();
    
    println!("=== Migration Test ===\n");
    
    // Create a temporary database for testing
    std::fs::create_dir_all("./data")?;
    let db_path = "./data/test_migration.db";
    if std::path::Path::new(db_path).exists() {
        std::fs::remove_file(db_path)?;
    }
    
    // Connect to database
    let db_url = format!("sqlite://{}?mode=rwc", db_path);
    println!("Connecting to database: {}", db_url);
    let db = Database::connect(&db_url).await?;
    
    // Run migrations
    println!("Running migrations...");
    Migrator::up(&db, None).await?;
    println!("✓ Migrations completed successfully!");
    
    // List tables to verify
    println!("\nCreated tables:");
    let result = db
        .query_all(Statement::from_string(
            sea_orm::DatabaseBackend::Sqlite,
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
        ))
        .await?;
    
    let tables: Vec<String> = result
        .into_iter()
        .filter_map(|row| row.try_get_by::<String, _>("name").ok())
        .collect();
    
    for table in tables {
        println!("  - {}", table);
    }
    
    // Clean up
    drop(db);
    std::fs::remove_file(db_path)?;
    
    Ok(())
}