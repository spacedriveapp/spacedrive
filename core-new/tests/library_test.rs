//! Integration tests for the library system

use sd_core_new::Core;
use tempfile::TempDir;

#[tokio::test]
async fn test_library_lifecycle() {
    // Create temporary directory for test
    let temp_dir = TempDir::new().unwrap();
    
    // Initialize core
    let core = Core::new().await.unwrap();
    
    // Add temp directory as search path
    let mut manager = core.libraries.clone();
    // Note: In real implementation, we'd make add_search_path accessible
    
    // Create library
    let library = core.libraries
        .create_library("Test Library", Some(temp_dir.path().to_path_buf()))
        .await
        .unwrap();
    
    assert_eq!(library.name().await, "Test Library");
    
    // Verify directory structure
    let lib_path = library.path();
    assert!(lib_path.exists());
    assert!(lib_path.join("library.json").exists());
    assert!(lib_path.join("database.db").exists());
    assert!(lib_path.join("thumbnails").exists());
    assert!(lib_path.join("thumbnails/metadata.json").exists());
    
    // Test thumbnail operations
    let cas_id = "test123";
    let thumb_data = b"test thumbnail data";
    
    library.save_thumbnail(cas_id, thumb_data).await.unwrap();
    assert!(library.has_thumbnail(cas_id).await);
    
    let retrieved = library.get_thumbnail(cas_id).await.unwrap();
    assert_eq!(retrieved, thumb_data);
    
    // Test configuration update
    library.update_config(|config| {
        config.description = Some("Test description".to_string());
        config.settings.thumbnail_quality = 90;
    }).await.unwrap();
    
    let config = library.config().await;
    assert_eq!(config.description, Some("Test description".to_string()));
    assert_eq!(config.settings.thumbnail_quality, 90);
    
    // Close library
    let lib_id = library.id();
    core.libraries.close_library(lib_id).await.unwrap();
    
    // Verify can't close again
    assert!(core.libraries.close_library(lib_id).await.is_err());
    
    // Re-open library
    let reopened = core.libraries.open_library(lib_path).await.unwrap();
    assert_eq!(reopened.id(), lib_id);
    assert_eq!(reopened.name().await, "Test Library");
    
    // Verify data persisted
    assert!(reopened.has_thumbnail(cas_id).await);
    let config = reopened.config().await;
    assert_eq!(config.description, Some("Test description".to_string()));
}

#[tokio::test]
async fn test_library_locking() {
    let temp_dir = TempDir::new().unwrap();
    let core = Core::new().await.unwrap();
    
    // Create library
    let library = core.libraries
        .create_library("Lock Test", Some(temp_dir.path().to_path_buf()))
        .await
        .unwrap();
    
    let lib_path = library.path().to_path_buf();
    
    // Try to open same library again - should fail
    let result = core.libraries.open_library(&lib_path).await;
    assert!(result.is_err());
    
    // Close library
    core.libraries.close_library(library.id()).await.unwrap();
    
    // Now should be able to open
    let reopened = core.libraries.open_library(&lib_path).await.unwrap();
    assert_eq!(reopened.name().await, "Lock Test");
}

#[tokio::test]
async fn test_library_discovery() {
    let temp_dir = TempDir::new().unwrap();
    let core = Core::new().await.unwrap();
    
    // Create multiple libraries
    let lib1 = core.libraries
        .create_library("Library 1", Some(temp_dir.path().to_path_buf()))
        .await
        .unwrap();
    
    let lib2 = core.libraries
        .create_library("Library 2", Some(temp_dir.path().to_path_buf()))
        .await
        .unwrap();
    
    // Close both
    core.libraries.close_library(lib1.id()).await.unwrap();
    core.libraries.close_library(lib2.id()).await.unwrap();
    
    // Scan for libraries
    // Note: In real implementation, we'd need to add temp_dir to search paths
    let discovered = core.libraries.scan_for_libraries().await.unwrap();
    
    // Should find at least our two libraries
    let names: Vec<String> = discovered.iter()
        .map(|d| d.config.name.clone())
        .collect();
    
    assert!(names.iter().any(|n| n.contains("Library 1")));
    assert!(names.iter().any(|n| n.contains("Library 2")));
}

#[tokio::test]
async fn test_library_name_sanitization() {
    let temp_dir = TempDir::new().unwrap();
    let core = Core::new().await.unwrap();
    
    // Create library with problematic name
    let library = core.libraries
        .create_library("My/Library:Name*", Some(temp_dir.path().to_path_buf()))
        .await
        .unwrap();
    
    // Verify directory name was sanitized
    let dir_name = library.path().file_name().unwrap().to_str().unwrap();
    assert!(dir_name.ends_with(".sdlibrary"));
    assert!(!dir_name.contains('/'));
    assert!(!dir_name.contains(':'));
    assert!(!dir_name.contains('*'));
}