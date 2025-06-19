//! Simple database test without library manager

use sd_core_new::infrastructure::database::{Database, entities};
use sea_orm::{EntityTrait, Set, ActiveModelTrait, PaginatorTrait};
use tempfile::TempDir;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Simple Database Test ===\n");

    // Create temporary directory
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("test.db");
    
    // Create and migrate database
    println!("1. Creating database...");
    let db = Database::create(&db_path).await?;
    println!("   ✓ Database created at {:?}", db_path);
    
    println!("\n2. Running migrations...");
    db.migrate().await?;
    println!("   ✓ Migrations completed");
    
    // Create a test library record
    println!("\n3. Inserting test data...");
    let library_id = Uuid::new_v4();
    
    let library = entities::library::ActiveModel {
        id: Set(library_id),
        name: Set("Test Library".to_string()),
        description: Set(Some("A test library".to_string())),
        encryption_algo: Set(None),
        hashing_algo: Set(Some("blake3".to_string())),
        path: Set(temp_dir.path().to_string_lossy().to_string()),
        version: Set("0.1.0".to_string()),
        created_at: Set(chrono::Utc::now()),
        updated_at: Set(chrono::Utc::now()),
    };
    
    let inserted_library = library.insert(db.conn()).await?;
    println!("   ✓ Inserted library: {} ({})", inserted_library.name, inserted_library.id);
    
    // Query it back
    println!("\n4. Querying data...");
    let found_library = entities::library::Entity::find_by_id(library_id)
        .one(db.conn())
        .await?;
    
    if let Some(lib) = found_library {
        println!("   ✓ Found library: {}", lib.name);
        println!("     - ID: {}", lib.id);
        println!("     - Path: {}", lib.path);
        println!("     - Created: {}", lib.created_at);
    } else {
        println!("   ✗ Library not found!");
    }
    
    // Count total libraries
    let count = entities::library::Entity::find()
        .count(db.conn())
        .await?;
    println!("   ✓ Total libraries: {}", count);
    
    println!("\n✅ Database test completed successfully!");
    
    Ok(())
}