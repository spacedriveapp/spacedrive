//! Example demonstrating the new file type identification system

use sd_core_new::file_type::FileTypeRegistry;
use sd_core_new::domain::ContentKind;
use std::path::Path;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create registry with built-in types
    let registry = FileTypeRegistry::new();
    
    println!("=== File Type Identification Demo ===\n");
    
    // Example 1: Simple extension matching
    println!("1. Extension matching:");
    let jpg_types = registry.get_by_extension("jpg");
    for ft in jpg_types {
        println!("  Found: {} ({})", ft.name, ft.id);
        println!("  MIME: {:?}", ft.mime_types);
        println!("  Category: {:?}", ft.category);
    }
    
    // Example 2: MIME type lookup
    println!("\n2. MIME type lookup:");
    if let Some(ft) = registry.get_by_mime("image/png") {
        println!("  image/png -> {} ({})", ft.name, ft.id);
    }
    
    // Example 3: Extension conflicts
    println!("\n3. Extension conflicts:");
    let ts_types = registry.get_by_extension("ts");
    println!("  '.ts' matches {} file types:", ts_types.len());
    for ft in ts_types {
        println!("    - {} ({}) priority={}", ft.name, ft.category as u8, ft.priority);
    }
    
    // Example 4: File identification (would use magic bytes)
    println!("\n4. File identification simulation:");
    
    // Simulate identifying a TypeScript file
    println!("  Identifying 'app.ts':");
    let ts_candidates = registry.get_by_extension("ts");
    for (i, ft) in ts_candidates.iter().enumerate() {
        println!("    Candidate {}: {} (priority={})", i+1, ft.name, ft.priority);
        if !ft.magic_bytes.is_empty() {
            println!("      Has {} magic byte patterns", ft.magic_bytes.len());
        } else {
            println!("      No magic bytes (text file)");
        }
    }
    
    // In real usage with a file:
    // let result = registry.identify(Path::new("video.ts")).await?;
    // println!("Identified as: {} with {}% confidence", result.file_type.name, result.confidence);
    
    // Example 5: Rich metadata
    println!("\n5. File type metadata:");
    if let Some(jpeg) = registry.get("image/jpeg") {
        println!("  JPEG metadata: {}", serde_json::to_string_pretty(&jpeg.metadata)?);
    }
    
    // Example 6: Integration with domain model
    println!("\n6. Domain model integration:");
    if let Some(png) = registry.get("image/png") {
        println!("  PNG uses ContentKind::{:?} directly", png.category);
        
        // Create a ContentIdentity from file type
        let mut content = sd_core_new::domain::ContentIdentity::new("cas:example".to_string(), 2);
        content.set_from_file_type(png);
        println!("  ContentIdentity kind: {:?}", content.kind);
    }
    
    Ok(())
}