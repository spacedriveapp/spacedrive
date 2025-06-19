//! Test database functionality - library creation and basic operations

use sd_core_new::{
    library::{LibraryManager, Library},
    infrastructure::events::EventBus,
    infrastructure::database::entities,
};
use sea_orm::{EntityTrait, Set, ActiveModelTrait};
use std::sync::Arc;
use tempfile::TempDir;
use tracing::{info, Level};
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    println!("=== Spacedrive Database Test ===\n");

    // Create temporary directory for test
    let temp_dir = TempDir::new()?;
    let libraries_path = temp_dir.path().join("Libraries");
    
    // Create event bus
    let event_bus = Arc::new(EventBus::new(100));
    
    // Create library manager
    let mut manager = LibraryManager::new(event_bus.clone());
    manager.add_search_path(libraries_path);
    
    // Test 1: Create a new library
    println!("1. Creating new library...");
    let library = manager.create_library("Test Library", None).await?;
    println!("   ✓ Created library: {}", library.id());
    println!("   ✓ Path: {:?}", library.path());
    
    // Test 2: Verify database was created and migrated
    println!("\n2. Testing database connection...");
    let db = library.db();
    
    // Insert test data - Create a device
    let device_id = Uuid::new_v4();
    let device = entities::device::ActiveModel {
        id: Set(device_id),
        library_id: Set(library.id()),
        name: Set("Test Device".to_string()),
        os: Set("macos".to_string()),
        os_version: Set(Some("14.0".to_string())),
        hardware_model: Set(Some("MacBook Pro".to_string())),
        network_addresses: Set(serde_json::json!(["192.168.1.100"])),
        is_online: Set(true),
        last_seen_at: Set(chrono::Utc::now()),
        capabilities: Set(serde_json::json!({
            "file_sync": true,
            "media_processing": true,
            "p2p": true
        })),
        created_at: Set(chrono::Utc::now()),
        updated_at: Set(chrono::Utc::now()),
    };
    
    let inserted_device = device.insert(db.conn()).await?;
    println!("   ✓ Inserted device: {}", inserted_device.name);
    
    // Test 3: Create a location
    println!("\n3. Creating location...");
    let location_id = Uuid::new_v4();
    let location = entities::location::ActiveModel {
        id: Set(location_id),
        library_id: Set(library.id()),
        device_id: Set(device_id),
        path: Set("/Users/test/Documents".to_string()),
        name: Set(Some("Documents".to_string())),
        index_mode: Set("content".to_string()),
        scan_state: Set("pending".to_string()),
        last_scan_at: Set(None),
        error_message: Set(None),
        total_file_count: Set(0),
        total_byte_size: Set(0),
        created_at: Set(chrono::Utc::now()),
        updated_at: Set(chrono::Utc::now()),
    };
    
    let inserted_location = location.insert(db.conn()).await?;
    println!("   ✓ Created location: {:?}", inserted_location.path);
    
    // Test 4: Create user metadata and entry
    println!("\n4. Creating file entry with metadata...");
    
    // First create user metadata (always exists!)
    let metadata_id = Uuid::new_v4();
    let metadata = entities::user_metadata::ActiveModel {
        id: Set(metadata_id),
        notes: Set(Some("Important document".to_string())),
        favorite: Set(true),
        hidden: Set(false),
        custom_data: Set(serde_json::json!({})),
        created_at: Set(chrono::Utc::now()),
        updated_at: Set(chrono::Utc::now()),
    };
    
    let inserted_metadata = metadata.insert(db.conn()).await?;
    println!("   ✓ Created user metadata (favorite: {})", inserted_metadata.favorite);
    
    // Then create the entry
    let entry_id = Uuid::new_v4();
    let entry = entities::entry::ActiveModel {
        id: Set(entry_id),
        sd_path: Set(format!("device:{}/documents/test.pdf", device_id)),
        name: Set("test.pdf".to_string()),
        kind: Set("file".to_string()),
        metadata_id: Set(metadata_id), // Always has metadata!
        content_id: Set(None), // No content identity yet
        location_id: Set(Some(location_id)),
        parent_id: Set(None),
        size: Set(1024000), // 1MB
        created_at: Set(chrono::Utc::now()),
        modified_at: Set(chrono::Utc::now()),
        accessed_at: Set(Some(chrono::Utc::now())),
        permissions: Set(Some("rw-r--r--".to_string())),
    };
    
    let inserted_entry = entry.insert(db.conn()).await?;
    println!("   ✓ Created entry: {}", inserted_entry.name);
    println!("   ✓ SdPath: {}", inserted_entry.sd_path);
    
    // Test 5: Query data back
    println!("\n5. Querying data...");
    
    // Count devices
    let device_count = entities::Device::find()
        .count(db.conn())
        .await?;
    println!("   ✓ Device count: {}", device_count);
    
    // Count entries
    let entry_count = entities::Entry::find()
        .count(db.conn())
        .await?;
    println!("   ✓ Entry count: {}", entry_count);
    
    // Find all locations
    let locations = entities::Location::find()
        .all(db.conn())
        .await?;
    println!("   ✓ Locations: {}", locations.len());
    for loc in locations {
        println!("     - {} ({})", loc.path, loc.scan_state);
    }
    
    // Test 6: Close and reopen library
    println!("\n6. Testing library persistence...");
    let library_id = library.id();
    let library_path = library.path().clone();
    
    // Close library
    manager.close_library(library_id).await?;
    println!("   ✓ Closed library");
    
    // Reopen library
    let reopened = manager.open_library(&library_path).await?;
    println!("   ✓ Reopened library: {}", reopened.id());
    
    // Verify data persisted
    let entry_count = entities::Entry::find()
        .count(reopened.db().conn())
        .await?;
    println!("   ✓ Entries after reopen: {}", entry_count);
    
    println!("\n✅ All tests passed!");
    
    Ok(())
}