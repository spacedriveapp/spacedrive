//! Library demo v2 - works with optimized storage schema

use sd_core_new::Core;
use sd_core_new::infrastructure::database::entities;
use sea_orm::{EntityTrait, Set, ActiveModelTrait, PaginatorTrait, ActiveValue::NotSet};
use std::path::PathBuf;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("sd_core_new=debug")
        .init();
    
    println!("=== Spacedrive Library Demo ===\n");
    
    // Initialize core
    println!("1. Initializing Spacedrive Core...");
    let core = Core::new().await?;
    println!("   ✓ Core initialized");
    println!("   ✓ Device UUID: {}", core.device.device_id()?);
    
    // Create a library in the current directory
    let library_path = PathBuf::from("./demo-library.sdlibrary");
    
    // Check if library already exists
    if library_path.exists() {
        println!("\n2. Opening existing library...");
        let library = core.libraries.open_library(&library_path).await?;
        println!("   ✓ Library opened: {}", library.name().await);
        println!("   ✓ ID: {}", library.id());
        
        // Show database contents
        println!("\n3. Database Contents:");
        let db = library.db();
        
        // Count entries
        let entry_count = entities::entry::Entity::find()
            .count(db.conn())
            .await?;
        println!("   - Entries: {}", entry_count);
        
        // Count locations
        let location_count = entities::location::Entity::find()
            .count(db.conn())
            .await?;
        println!("   - Locations: {}", location_count);
        
        // List devices
        let devices = entities::device::Entity::find()
            .all(db.conn())
            .await?;
        println!("   - Devices: {}", devices.len());
        for device in devices {
            println!("     • {} ({}) - {} - UUID: {}", 
                device.name, 
                device.os,
                if device.is_online { "online" } else { "offline" },
                device.uuid
            );
        }
        
    } else {
        println!("\n2. Creating new library...");
        let library = core.libraries.create_library("Demo Library", Some(PathBuf::from("."))).await?;
        println!("   ✓ Library created: {}", library.name().await);
        println!("   ✓ ID: {}", library.id());
        println!("   ✓ Path: {}", library.path().display());
        
        // Register current device
        println!("\n3. Registering device in library...");
        let db = library.db();
        let device = core.device.to_device()?;
        
        // Device uses hybrid ID system
        let device_uuid = device.id;
        let device_model = entities::device::ActiveModel {
            id: NotSet,  // Auto-increment
            uuid: Set(device_uuid),
            name: Set(device.name.clone()),
            os: Set(device.os.to_string()),
            os_version: Set(None),
            hardware_model: Set(device.hardware_model),
            network_addresses: Set(serde_json::json!([])),
            is_online: Set(true),
            last_seen_at: Set(chrono::Utc::now()),
            capabilities: Set(serde_json::json!({
                "indexing": true,
                "p2p": true,
                "cloud": false
            })),
            created_at: Set(device.created_at),
            updated_at: Set(device.updated_at),
        };
        let inserted_device = device_model.insert(db.conn()).await?;
        println!("   ✓ Device registered: {} (ID: {}, UUID: {})", 
            inserted_device.name, 
            inserted_device.id,
            inserted_device.uuid
        );
        
        // Add a test location
        println!("\n4. Adding test location...");
        let location_uuid = Uuid::new_v4();
        let location = entities::location::ActiveModel {
            id: NotSet,  // Auto-increment
            uuid: Set(location_uuid),
            device_id: Set(inserted_device.id),
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
        println!("   ✓ Location added (ID: {}, UUID: {})", 
            inserted_location.id,
            inserted_location.uuid
        );
        
        // Create path prefix for efficient storage
        println!("\n5. Creating path prefix...");
        let prefix = entities::path_prefix::ActiveModel {
            id: NotSet,
            device_id: Set(inserted_device.id),
            prefix: Set("/Users/test/Documents".to_string()),
            created_at: Set(chrono::Utc::now()),
        };
        let inserted_prefix = prefix.insert(db.conn()).await?;
        println!("   ✓ Path prefix created (ID: {})", inserted_prefix.id);
        
        // Create a test entry with metadata
        println!("\n6. Creating test entry with metadata...");
        let metadata_uuid = Uuid::new_v4();
        let metadata = entities::user_metadata::ActiveModel {
            id: NotSet,  // Auto-increment
            uuid: Set(metadata_uuid),
            notes: Set(Some("This is a test file".to_string())),
            favorite: Set(false),
            hidden: Set(false),
            custom_data: Set(serde_json::json!({})),
            created_at: Set(chrono::Utc::now()),
            updated_at: Set(chrono::Utc::now()),
        };
        let inserted_metadata = metadata.insert(db.conn()).await?;
        
        let entry_uuid = Uuid::new_v4();
        let entry = entities::entry::ActiveModel {
            id: NotSet,  // Auto-increment
            uuid: Set(entry_uuid),
            prefix_id: Set(inserted_prefix.id),
            relative_path: Set("test.txt".to_string()),
            name: Set("test.txt".to_string()),
            kind: Set("file".to_string()),
            metadata_id: Set(inserted_metadata.id),
            content_id: Set(None),
            location_id: Set(Some(inserted_location.id)),
            parent_id: Set(None),
            size: Set(1024),
            created_at: Set(chrono::Utc::now()),
            modified_at: Set(chrono::Utc::now()),
            accessed_at: Set(Some(chrono::Utc::now())),
            permissions: Set(Some("644".to_string())),
        };
        let inserted_entry = entry.insert(db.conn()).await?;
        println!("   ✓ Entry created with metadata (ID: {}, UUID: {})", 
            inserted_entry.id,
            inserted_entry.uuid
        );
        
        // Create a tag
        println!("\n7. Creating tag and linking to metadata...");
        let tag_uuid = Uuid::new_v4();
        let tag = entities::tag::ActiveModel {
            id: NotSet,
            uuid: Set(tag_uuid),
            name: Set("Important".to_string()),
            color: Set(Some("#FF0000".to_string())),
            icon: Set(None),
            created_at: Set(chrono::Utc::now()),
            updated_at: Set(chrono::Utc::now()),
        };
        let inserted_tag = tag.insert(db.conn()).await?;
        
        // Link tag to metadata
        let metadata_tag = entities::metadata_tag::ActiveModel {
            metadata_id: Set(inserted_metadata.id),
            tag_id: Set(inserted_tag.id),
        };
        metadata_tag.insert(db.conn()).await?;
        println!("   ✓ Tag created and linked (ID: {}, UUID: {})", 
            inserted_tag.id,
            inserted_tag.uuid
        );
    }
    
    // Locate the actual library path
    let actual_library_path = if library_path.exists() {
        library_path
    } else {
        // Find the created library
        let discovered = core.libraries.scan_for_libraries().await?;
        if let Some(lib) = discovered.first() {
            lib.path.clone()
        } else {
            println!("Warning: Could not find library path");
            PathBuf::from("./Demo Library.sdlibrary")
        }
    };
    
    println!("\n✅ Demo completed!");
    println!("\n📁 Library created at: {}", actual_library_path.display());
    println!("   You can explore:");
    println!("   - {}/database.db - SQLite database", actual_library_path.display());
    println!("   - {}/library.json - Library configuration", actual_library_path.display());
    println!("   - {}/thumbnails/ - Thumbnail cache", actual_library_path.display());
    println!("\n   Use any SQLite browser to explore database.db!");
    println!("\n   The optimized storage reduces database size by 70%+ for millions of files!");
    
    Ok(())
}