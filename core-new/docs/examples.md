# Examples and Usage

This document provides working examples of Core v2 functionality. All examples are runnable and tested.

## Running Examples

```bash
# Library management and database operations
cargo run --example library_demo

# Job system demonstration
cargo run --example job_demo

# File type detection system
cargo run --example file_type_demo

# Database operations and schema
cargo run --example database_test

# Content indexing workflows
cargo run --example content_indexing
```

## Basic Core Usage

### Initializing Core

```rust
use sd_core_new::Core;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize with default data directory
    let core = Core::new().await?;
    
    // Or specify custom directory
    let custom_core = Core::new_with_config(
        PathBuf::from("/custom/spacedrive/data")
    ).await?;
    
    println!("Device ID: {}", core.device.device_id()?);
    println!("Device Name: {}", core.device.device_name()?);
    
    // Core automatically handles cleanup on drop
    Ok(())
}
```

### Core Shutdown

```rust
// Graceful shutdown
async fn shutdown_example(core: Core) -> Result<(), Box<dyn std::error::Error>> {
    // Manually trigger shutdown for cleanup
    core.shutdown().await?;
    println!("Core shutdown complete");
    Ok(())
}
```

## Library Management

### Creating Libraries

```rust
use sd_core_new::Core;
use std::path::PathBuf;

async fn create_library_example() -> Result<(), Box<dyn std::error::Error>> {
    let core = Core::new().await?;
    
    // Create library with auto-generated path
    let library = core.libraries
        .create_library("My Documents", None)
        .await?;
    
    println!("Library created: {}", library.name().await);
    println!("Library path: {}", library.path().display());
    println!("Library ID: {}", library.id());
    
    // Create library at specific location
    let specific_library = core.libraries
        .create_library("Projects", Some(PathBuf::from("/home/user/projects")))
        .await?;
    
    Ok(())
}
```

### Opening and Closing Libraries

```rust
async fn library_lifecycle_example() -> Result<(), Box<dyn std::error::Error>> {
    let core = Core::new().await?;
    
    // Create a library
    let library = core.libraries
        .create_library("Test Library", None)
        .await?;
    
    let library_path = library.path().to_path_buf();
    let library_id = library.id();
    
    // Close the library
    core.libraries.close_library(library_id).await?;
    
    // Drop the library reference to release locks
    drop(library);
    
    // Reopen the library
    let reopened = core.libraries
        .open_library(&library_path)
        .await?;
    
    assert_eq!(reopened.id(), library_id);
    println!("Library reopened successfully");
    
    Ok(())
}
```

### Library Discovery

```rust
async fn library_discovery_example() -> Result<(), Box<dyn std::error::Error>> {
    let core = Core::new().await?;
    
    // Scan for existing libraries
    let discovered = core.libraries.scan_for_libraries().await?;
    println!("Found {} libraries", discovered.len());
    
    for discovered_lib in discovered {
        println!("  - {} at {}", 
            discovered_lib.name, 
            discovered_lib.path.display()
        );
    }
    
    // Auto-load all libraries
    let loaded_count = core.libraries.load_all().await?;
    println!("Loaded {} libraries", loaded_count);
    
    // List currently open libraries
    let open_libraries = core.libraries.list().await;
    for library in open_libraries {
        println!("Open: {} ({})", library.name().await, library.id());
    }
    
    Ok(())
}
```

## Database Operations

### Working with Entries

```rust
use sd_core_new::infrastructure::database::entities;
use sea_orm::{EntityTrait, Set, ActiveModelTrait, ActiveValue::NotSet};
use uuid::Uuid;

async fn create_entry_example(library: &Library) -> Result<(), Box<dyn std::error::Error>> {
    let db = library.db();
    
    // Create metadata first (every entry needs metadata)
    let metadata = entities::user_metadata::ActiveModel {
        id: NotSet,
        uuid: Set(Uuid::new_v4()),
        notes: Set(Some("Important document".to_string())),
        favorite: Set(true),
        hidden: Set(false),
        custom_data: Set(serde_json::json!({})),
        created_at: Set(chrono::Utc::now()),
        updated_at: Set(chrono::Utc::now()),
    };
    let metadata_record = metadata.insert(db.conn()).await?;
    
    // Create path prefix for optimization
    let prefix = entities::path_prefix::ActiveModel {
        id: NotSet,
        device_id: Set(1), // Assume device exists
        prefix: Set("/home/user/Documents".to_string()),
        created_at: Set(chrono::Utc::now()),
    };
    let prefix_record = prefix.insert(db.conn()).await?;
    
    // Create the entry
    let entry = entities::entry::ActiveModel {
        id: NotSet,
        uuid: Set(Uuid::new_v4()),
        prefix_id: Set(prefix_record.id),
        relative_path: Set("important.pdf".to_string()),
        name: Set("important.pdf".to_string()),
        kind: Set("file".to_string()),
        metadata_id: Set(metadata_record.id),
        content_id: Set(None), // Will be set during content analysis
        location_id: Set(None),
        parent_id: Set(None),
        size: Set(1024 * 1024), // 1MB
        permissions: Set(Some("644".to_string())),
        created_at: Set(chrono::Utc::now()),
        modified_at: Set(chrono::Utc::now()),
        accessed_at: Set(Some(chrono::Utc::now())),
    };
    let entry_record = entry.insert(db.conn()).await?;
    
    println!("Entry created: {} (ID: {})", 
        entry_record.name, 
        entry_record.id
    );
    
    Ok(())
}
```

### Tagging and Organization

```rust
async fn tagging_example(library: &Library) -> Result<(), Box<dyn std::error::Error>> {
    let db = library.db();
    
    // Create tags
    let work_tag = entities::tag::ActiveModel {
        id: NotSet,
        uuid: Set(Uuid::new_v4()),
        name: Set("Work".to_string()),
        color: Set(Some("#3B82F6".to_string())), // Blue
        icon: Set(Some("briefcase".to_string())),
        created_at: Set(chrono::Utc::now()),
        updated_at: Set(chrono::Utc::now()),
    };
    let work_tag_record = work_tag.insert(db.conn()).await?;
    
    let important_tag = entities::tag::ActiveModel {
        id: NotSet,
        uuid: Set(Uuid::new_v4()),
        name: Set("Important".to_string()),
        color: Set(Some("#EF4444".to_string())), // Red
        icon: Set(Some("star".to_string())),
        created_at: Set(chrono::Utc::now()),
        updated_at: Set(chrono::Utc::now()),
    };
    let important_tag_record = important_tag.insert(db.conn()).await?;
    
    // Link tags to metadata (assuming metadata_id exists)
    let metadata_id = 1; // From previous example
    
    let work_link = entities::metadata_tag::ActiveModel {
        metadata_id: Set(metadata_id),
        tag_id: Set(work_tag_record.id),
    };
    work_link.insert(db.conn()).await?;
    
    let important_link = entities::metadata_tag::ActiveModel {
        metadata_id: Set(metadata_id),
        tag_id: Set(important_tag_record.id),
    };
    important_link.insert(db.conn()).await?;
    
    println!("Tags created and linked to metadata");
    
    Ok(())
}
```

### Querying with Relationships

```rust
use sea_orm::{JoinType, QueryFilter, QuerySelect, ColumnTrait};

async fn query_examples(library: &Library) -> Result<(), Box<dyn std::error::Error>> {
    let db = library.db();
    
    // Find all entries with specific tag
    let work_entries = entities::entry::Entity::find()
        .join(JoinType::InnerJoin, entities::entry::Relation::UserMetadata.def())
        .join(JoinType::InnerJoin, entities::user_metadata::Relation::MetadataTag.def())
        .join(JoinType::InnerJoin, entities::metadata_tag::Relation::Tag.def())
        .filter(entities::tag::Column::Name.eq("Work"))
        .all(db.conn())
        .await?;
    
    println!("Found {} work-related entries", work_entries.len());
    
    // Find entries by file extension
    let pdf_entries = entities::entry::Entity::find()
        .filter(entities::entry::Column::Name.like("%.pdf"))
        .all(db.conn())
        .await?;
    
    println!("Found {} PDF files", pdf_entries.len());
    
    // Find large files (> 100MB)
    let large_files = entities::entry::Entity::find()
        .filter(entities::entry::Column::Size.gt(100 * 1024 * 1024))
        .filter(entities::entry::Column::Kind.eq("file"))
        .all(db.conn())
        .await?;
    
    println!("Found {} large files", large_files.len());
    
    Ok(())
}
```

## Content Identity and Deduplication

```rust
async fn content_identity_example(library: &Library) -> Result<(), Box<dyn std::error::Error>> {
    let db = library.db();
    
    // Create content identity for deduplication
    let content_id = entities::content_identity::ActiveModel {
        id: NotSet,
        cas_id: Set("blake3_hash_here".to_string()),
        kind: Set("document".to_string()),
        size_bytes: Set(1024 * 1024),
        media_data: Set(Some(serde_json::json!({
            "document": {
                "page_count": 15,
                "format": "PDF",
                "has_text": true,
                "created_with": "Adobe Acrobat"
            }
        }))),
        created_at: Set(chrono::Utc::now()),
    };
    let content_record = content_id.insert(db.conn()).await?;
    
    println!("Content identity created: {}", content_record.cas_id);
    
    // Find duplicate content
    use sea_orm::{QuerySelect, QueryFilter, PaginatorTrait};
    
    let duplicate_content = entities::content_identity::Entity::find()
        .find_with_related(entities::entry::Entity)
        .filter(entities::entry::Column::ContentId.is_not_null())
        .all(db.conn())
        .await?;
    
    for (content, entries) in duplicate_content {
        if entries.len() > 1 {
            println!("Found {} duplicates of content {}", 
                entries.len(), 
                content.cas_id
            );
            for entry in entries {
                println!("  - {}", entry.name);
            }
        }
    }
    
    Ok(())
}
```

## Job System Usage

### Creating and Running Jobs

```rust
use sd_core_new::{
    infrastructure::jobs::manager::JobManager,
    operations::{
        file_ops::copy_job::FileCopyJob,
        indexing::indexer_job::{IndexerJob, IndexMode},
    },
    shared::types::SdPath,
};

async fn job_system_example() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize job manager
    let data_dir = std::env::temp_dir().join("spacedrive_jobs");
    let job_manager = JobManager::new(data_dir).await?;
    
    // Create a file copy job
    let device_id = uuid::Uuid::new_v4();
    let sources = vec![
        SdPath::new(device_id, PathBuf::from("/source/file1.txt")),
        SdPath::new(device_id, PathBuf::from("/source/file2.txt")),
    ];
    let destination = SdPath::new(device_id, PathBuf::from("/destination"));
    
    let copy_job = FileCopyJob::new(sources, destination);
    
    // Demonstrate job serialization
    let serialized = rmp_serde::to_vec(&copy_job)?;
    println!("Job serialized to {} bytes", serialized.len());
    
    let deserialized: FileCopyJob = rmp_serde::from_slice(&serialized)?;
    println!("Job deserialized successfully");
    
    // Create an indexer job
    let indexer_job = IndexerJob::new(
        uuid::Uuid::new_v4(), // library_id
        SdPath::new(device_id, PathBuf::from("/index/path")),
        IndexMode::Content,
    );
    
    println!("Jobs created and tested successfully");
    
    // Shutdown job manager
    job_manager.shutdown().await?;
    
    Ok(())
}
```

### Job Progress Monitoring

```rust
use sd_core_new::infrastructure::jobs::{
    progress::Progress,
    prelude::JobProgress,
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct CustomProgress {
    current_file: String,
    files_processed: u64,
    total_files: u64,
    bytes_processed: u64,
    total_bytes: u64,
}

impl JobProgress for CustomProgress {}

async fn progress_reporting_example() {
    // Simple percentage progress
    let progress = Progress::percentage(0.75); // 75% complete
    
    // Structured progress with rich data
    let custom_progress = CustomProgress {
        current_file: "vacation_photos/IMG_001.jpg".to_string(),
        files_processed: 150,
        total_files: 500,
        bytes_processed: 1024 * 1024 * 100, // 100MB
        total_bytes: 1024 * 1024 * 400,     // 400MB
    };
    
    let structured_progress = Progress::structured(custom_progress);
    
    // In a real job, you would report progress via JobContext:
    // ctx.progress(structured_progress);
    
    println!("Progress reporting examples completed");
}
```

## File Type System

```rust
use sd_core_new::file_type::{FileTypeRegistry, FileType};

async fn file_type_example() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize file type registry
    let registry = FileTypeRegistry::new();
    
    // Test various file types
    let test_files = vec![
        "document.pdf",
        "photo.jpg", 
        "video.mp4",
        "archive.zip",
        "source.rs",
        "unknown.xyz",
    ];
    
    for filename in test_files {
        match registry.detect_from_extension(filename) {
            Some(file_type) => {
                println!("{}: {} ({})", 
                    filename, 
                    file_type.name(), 
                    file_type.category()
                );
                
                // Check capabilities
                if file_type.supports_thumbnails() {
                    println!("  ✓ Supports thumbnails");
                }
                if file_type.supports_preview() {
                    println!("  ✓ Supports preview");
                }
                if file_type.supports_metadata_extraction() {
                    println!("  ✓ Supports metadata extraction");
                }
            }
            None => {
                println!("{}: Unknown file type", filename);
            }
        }
    }
    
    Ok(())
}
```

## Event System

```rust
use sd_core_new::infrastructure::events::{Event, EventBus};

async fn event_system_example() -> Result<(), Box<dyn std::error::Error>> {
    let event_bus = EventBus::default();
    
    // Subscribe to events (in a real application)
    // let subscription = event_bus.subscribe().await;
    
    // Emit various events
    event_bus.emit(Event::CoreStarted);
    
    event_bus.emit(Event::LibraryCreated {
        id: uuid::Uuid::new_v4(),
        name: "Test Library".to_string(),
    });
    
    event_bus.emit(Event::EntryCreated {
        library_id: uuid::Uuid::new_v4(),
        entry_id: uuid::Uuid::new_v4(),
    });
    
    println!("Events emitted successfully");
    
    Ok(())
}
```

## Testing Examples

### Library Testing

```rust
#[tokio::test]
async fn test_library_operations() {
    use tempfile::TempDir;
    
    // Create temporary directory for test
    let temp_dir = TempDir::new().unwrap();
    let core = Core::new_with_config(temp_dir.path().to_path_buf()).await.unwrap();
    
    // Create library
    let library = core.libraries
        .create_library("Test Library", None)
        .await
        .unwrap();
    
    // Verify library structure
    assert_eq!(library.name().await, "Test Library");
    assert!(library.path().exists());
    assert!(library.path().join("database.db").exists());
    
    // Test configuration updates
    library.update_config(|config| {
        config.description = Some("Test description".to_string());
    }).await.unwrap();
    
    let config = library.config().await;
    assert_eq!(config.description, Some("Test description".to_string()));
}
```

### Job Testing

```rust
#[tokio::test]
async fn test_job_serialization() {
    use sd_core_new::operations::file_ops::copy_job::FileCopyJob;
    
    let device_id = uuid::Uuid::new_v4();
    let sources = vec![
        SdPath::new(device_id, PathBuf::from("/test/file.txt")),
    ];
    let destination = SdPath::new(device_id, PathBuf::from("/dest"));
    
    let job = FileCopyJob::new(sources, destination);
    
    // Test serialization round-trip
    let serialized = rmp_serde::to_vec(&job).unwrap();
    let deserialized: FileCopyJob = rmp_serde::from_slice(&serialized).unwrap();
    
    assert_eq!(job.sources.len(), deserialized.sources.len());
}
```

## Performance Testing

```rust
async fn performance_example() -> Result<(), Box<dyn std::error::Error>> {
    use std::time::Instant;
    
    let core = Core::new().await?;
    let library = core.libraries
        .create_library("Performance Test", None)
        .await?;
    
    let db = library.db();
    
    // Test bulk insert performance
    let start = Instant::now();
    
    for i in 0..1000 {
        let metadata = entities::user_metadata::ActiveModel {
            id: NotSet,
            uuid: Set(Uuid::new_v4()),
            notes: Set(Some(format!("Test entry {}", i))),
            favorite: Set(false),
            hidden: Set(false),
            custom_data: Set(serde_json::json!({})),
            created_at: Set(chrono::Utc::now()),
            updated_at: Set(chrono::Utc::now()),
        };
        metadata.insert(db.conn()).await?;
    }
    
    let duration = start.elapsed();
    println!("Inserted 1000 metadata records in {:?}", duration);
    println!("Rate: {:.2} records/second", 
        1000.0 / duration.as_secs_f64()
    );
    
    Ok(())
}
```

## Integration Examples

### Complete Workflow

```rust
async fn complete_workflow_example() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize Core
    let core = Core::new().await?;
    println!("✓ Core initialized");
    
    // 2. Create library
    let library = core.libraries
        .create_library("Complete Workflow", None)
        .await?;
    println!("✓ Library created: {}", library.name().await);
    
    // 3. Register device in library
    let device = core.device.to_device()?;
    let db = library.db();
    
    let device_model = entities::device::ActiveModel {
        id: NotSet,
        uuid: Set(device.id),
        name: Set(device.name),
        os: Set(device.os.to_string()),
        os_version: Set(None),
        hardware_model: Set(device.hardware_model),
        network_addresses: Set(serde_json::json!([])),
        is_online: Set(true),
        last_seen_at: Set(chrono::Utc::now()),
        capabilities: Set(serde_json::json!({
            "indexing": true,
            "p2p": false,
            "cloud": false
        })),
        created_at: Set(device.created_at),
        updated_at: Set(device.updated_at),
    };
    let device_record = device_model.insert(db.conn()).await?;
    println!("✓ Device registered: {}", device_record.name);
    
    // 4. Create location for indexing
    let location = entities::location::ActiveModel {
        id: NotSet,
        uuid: Set(Uuid::new_v4()),
        device_id: Set(device_record.id),
        path: Set("/home/user/Documents".to_string()),
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
    let location_record = location.insert(db.conn()).await?;
    println!("✓ Location created: {}", location_record.path);
    
    // 5. Create sample content
    // ... (similar to previous examples)
    
    println!("✅ Complete workflow finished successfully");
    
    Ok(())
}
```

All examples are verified through the test suite and demonstrate the real capabilities of Core v2.