//! Example showing how file type identification integrates with content indexing

use sd_core_new::{
    domain::{Entry, UserMetadata, ContentIdentity, Location, IndexMode},
    file_type::FileTypeRegistry,
};
use std::path::Path;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Content Indexing Workflow ===\n");
    
    // Initialize file type registry
    let registry = FileTypeRegistry::new();
    
    // Simulate indexing a location
    let location = Location {
        id: Uuid::new_v4(),
        path: "/Users/demo/Photos".to_string(),
        device_id: Uuid::new_v4(),
        index_mode: IndexMode::Content, // Content mode enables deduplication
        // ... other fields
    };
    
    println!("Indexing location: {}", location.path);
    println!("Mode: {:?}\n", location.index_mode);
    
    // Example 1: Index a JPEG file
    let jpeg_path = Path::new("/Users/demo/Photos/vacation.jpg");
    println!("1. Processing: {}", jpeg_path.display());
    
    // Step 1: Identify file type
    let identification = registry.identify(jpeg_path).await?;
    println!("   Identified as: {} ({}% confidence)", 
             identification.file_type.name, 
             identification.confidence);
    println!("   Category: {:?}", identification.file_type.category);
    println!("   MIME: {:?}", identification.file_type.primary_mime_type());
    
    // Step 2: Create entry with metadata (always exists)
    let mut entry = Entry {
        id: Uuid::new_v4(),
        sd_path: format!("device:{}/photos/vacation.jpg", location.device_id),
        name: "vacation.jpg".to_string(),
        kind: sd_core_new::domain::EntryKind::File,
        metadata_id: Uuid::new_v4(),
        content_id: None, // Will be set after content indexing
        location_id: Some(location.id),
        parent_id: None,
        size: 2_500_000, // 2.5MB
        created_at: chrono::Utc::now(),
        modified_at: chrono::Utc::now(),
        accessed_at: Some(chrono::Utc::now()),
        permissions: None,
    };
    
    // User metadata always exists
    let user_metadata = UserMetadata {
        id: entry.metadata_id,
        tags: vec![],
        labels: vec![],
        notes: Some("Family vacation in Hawaii".to_string()),
        favorite: true,
        hidden: false,
        custom_data: serde_json::json!({}),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    
    println!("   Created entry with metadata");
    println!("   - Favorite: {}", user_metadata.favorite);
    println!("   - Notes: {:?}", user_metadata.notes);
    
    // Step 3: Since IndexMode::Content, compute content identity
    if matches!(location.index_mode, IndexMode::Content | IndexMode::Deep) {
        println!("   Computing content identity...");
        
        // Simulate CAS ID computation
        let cas_id = "v2:1234567890abcdef".to_string();
        
        // Create content identity
        let mut content = ContentIdentity::new(cas_id.clone(), 2);
        
        // Use file type to set content kind and MIME
        content.set_from_file_type(&identification.file_type);
        content.add_size(entry.size);
        
        // In Deep mode, we'd extract more metadata
        if matches!(location.index_mode, IndexMode::Deep) {
            content.media_data = Some(sd_core_new::domain::MediaData {
                width: Some(3840),
                height: Some(2160),
                exif: Some(sd_core_new::domain::ExifData {
                    make: Some("Canon".to_string()),
                    model: Some("EOS R5".to_string()),
                    date_taken: Some(chrono::Utc::now()),
                    gps: Some(sd_core_new::domain::GpsCoordinates {
                        latitude: 21.3099,
                        longitude: -157.8581,
                        altitude: Some(10.0),
                    }),
                    iso: Some(200),
                    aperture: Some(2.8),
                    shutter_speed: Some(0.001),
                    focal_length: Some(50.0),
                }),
                duration: None,
                bitrate: None,
                fps: None,
                extra: serde_json::json!({}),
            });
        }
        
        // Link entry to content
        entry.content_id = Some(content.id);
        
        println!("   Content identity created:");
        println!("   - CAS ID: {}", content.cas_id);
        println!("   - Kind: {:?}", content.kind);
        println!("   - MIME: {:?}", content.mime_type);
        if let Some(ref media) = content.media_data {
            if let Some(ref exif) = media.exif {
                println!("   - Camera: {:?} {:?}", exif.make, exif.model);
                if let Some(ref gps) = exif.gps {
                    println!("   - Location: {:.4}, {:.4}", gps.latitude, gps.longitude);
                }
            }
        }
    }
    
    // Example 2: Process a duplicate file
    println!("\n2. Processing duplicate: /Users/demo/Documents/vacation-copy.jpg");
    
    let dup_path = Path::new("/Users/demo/Documents/vacation-copy.jpg");
    let dup_identification = registry.identify(dup_path).await?;
    
    // Compute CAS ID - would match the original
    let dup_cas_id = "v2:1234567890abcdef".to_string(); // Same as original!
    
    println!("   CAS ID matches existing content!");
    println!("   - Linking to existing ContentIdentity");
    println!("   - Incrementing entry_count to 2");
    println!("   - No duplicate storage needed");
    
    // Example 3: Different file types
    println!("\n3. File type examples:");
    
    let examples = [
        ("document.pdf", "Document processing, text extraction"),
        ("movie.mp4", "Video metadata, thumbnails, duration"),
        ("song.mp3", "Audio metadata, ID3 tags, album art"),
        ("archive.zip", "Archive contents listing"),
        ("script.ts", "Code analysis, syntax highlighting"),
    ];
    
    for (filename, features) in &examples {
        let path = Path::new(filename);
        if let Ok(result) = registry.identify(path).await {
            println!("   {} -> {} ({})", 
                     filename, 
                     result.file_type.category as u8,
                     features);
        }
    }
    
    Ok(())
}