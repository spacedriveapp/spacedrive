//! Tests for Content Addressable Storage (CAS) ID generation

use sd_core_new::domain::content_identity::{
    CasGenerator, CasError, ContentIdentity, ContentKind, MediaData, ExifData, GpsCoordinates,
    CURRENT_CAS_VERSION, SMALL_FILE_THRESHOLD,
};
use std::io::Write;
use tempfile::TempDir;
use tokio::fs;

#[tokio::test]
async fn test_cas_small_file_generation() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("small_file.txt");
    
    // Create a small file (under threshold)
    let content = b"Hello, World! This is a small test file.";
    fs::write(&file_path, content).await.unwrap();
    
    let cas_id = CasGenerator::generate_cas_id(&file_path).await.unwrap();
    
    // Should be a full hash for small files
    assert!(cas_id.starts_with(&format!("v{}_full:", CURRENT_CAS_VERSION)));
    assert!(cas_id.len() > 20); // Should be a reasonable hash length
    
    // Same file should produce same CAS ID
    let cas_id2 = CasGenerator::generate_cas_id(&file_path).await.unwrap();
    assert_eq!(cas_id, cas_id2);
}

#[tokio::test]
async fn test_cas_large_file_generation() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("large_file.dat");
    
    // Create a large file (over threshold)
    let chunk_size = 1024 * 1024; // 1MB
    let num_chunks = (SMALL_FILE_THRESHOLD / chunk_size as u64) + 5; // Ensure it's over threshold
    
    let mut file = std::fs::File::create(&file_path).unwrap();
    let chunk = vec![b'A'; chunk_size];
    
    for i in 0..num_chunks {
        // Vary the content slightly to make it realistic
        let mut varied_chunk = chunk.clone();
        varied_chunk[0] = (i % 256) as u8;
        file.write_all(&varied_chunk).unwrap();
    }
    drop(file);
    
    let cas_id = CasGenerator::generate_cas_id(&file_path).await.unwrap();
    
    // Should be a sampled hash for large files
    assert!(cas_id.starts_with(&format!("v{}_sampled:", CURRENT_CAS_VERSION)));
    assert!(cas_id.len() > 20);
    
    // Same file should produce same CAS ID
    let cas_id2 = CasGenerator::generate_cas_id(&file_path).await.unwrap();
    assert_eq!(cas_id, cas_id2);
}

#[tokio::test]
async fn test_cas_empty_file() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("empty_file.txt");
    
    // Create empty file
    fs::write(&file_path, b"").await.unwrap();
    
    let cas_id = CasGenerator::generate_cas_id(&file_path).await.unwrap();
    
    // Empty file should still get a CAS ID
    assert!(cas_id.starts_with(&format!("v{}_full:", CURRENT_CAS_VERSION)));
    
    // All empty files should have the same CAS ID
    let file_path2 = temp_dir.path().join("empty_file2.txt");
    fs::write(&file_path2, b"").await.unwrap();
    
    let cas_id2 = CasGenerator::generate_cas_id(&file_path2).await.unwrap();
    assert_eq!(cas_id, cas_id2);
}

#[tokio::test]
async fn test_cas_content_based_generation() {
    // Test generating CAS ID from raw content
    let content1 = b"Test content for CAS ID generation";
    let content2 = b"Different test content";
    let content3 = b"Test content for CAS ID generation"; // Same as content1
    
    let cas_id1 = CasGenerator::generate_from_content(content1);
    let cas_id2 = CasGenerator::generate_from_content(content2);
    let cas_id3 = CasGenerator::generate_from_content(content3);
    
    // Same content should produce same CAS ID
    assert_eq!(cas_id1, cas_id3);
    
    // Different content should produce different CAS IDs
    assert_ne!(cas_id1, cas_id2);
    
    // All should be content-based
    assert!(cas_id1.starts_with(&format!("v{}_content:", CURRENT_CAS_VERSION)));
    assert!(cas_id2.starts_with(&format!("v{}_content:", CURRENT_CAS_VERSION)));
    assert!(cas_id3.starts_with(&format!("v{}_content:", CURRENT_CAS_VERSION)));
}

#[tokio::test]
async fn test_cas_verification() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("verify_test.txt");
    
    let content = b"Content for verification test";
    fs::write(&file_path, content).await.unwrap();
    
    let cas_id = CasGenerator::generate_cas_id(&file_path).await.unwrap();
    
    // Verification should succeed for correct CAS ID
    let is_valid = CasGenerator::verify_cas_id(&file_path, &cas_id).await.unwrap();
    assert!(is_valid);
    
    // Verification should fail for incorrect CAS ID
    let wrong_cas_id = "v2_full:wronghash";
    let is_valid = CasGenerator::verify_cas_id(&file_path, wrong_cas_id).await.unwrap();
    assert!(!is_valid);
    
    // Modify file and verification should fail
    fs::write(&file_path, b"Modified content").await.unwrap();
    let is_valid = CasGenerator::verify_cas_id(&file_path, &cas_id).await.unwrap();
    assert!(!is_valid);
}

#[tokio::test]
async fn test_cas_error_handling() {
    // Test with non-existent file
    let non_existent = std::path::Path::new("/non/existent/file.txt");
    let result = CasGenerator::generate_cas_id(non_existent).await;
    
    assert!(result.is_err());
    match result.unwrap_err() {
        CasError::Io(_) => {
            // Expected
        }
        _ => panic!("Expected IO error"),
    }
    
    // Test verification with non-existent file
    let result = CasGenerator::verify_cas_id(non_existent, "v2_full:test").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_cas_uniqueness() {
    let temp_dir = TempDir::new().unwrap();
    
    // Create multiple files with different content
    let files_and_content = vec![
        ("file1.txt", "Content 1"),
        ("file2.txt", "Content 2"),
        ("file3.txt", "Different content entirely"),
        ("file4.txt", "Content 1"), // Same as file1
    ];
    
    let mut cas_ids = Vec::new();
    
    for (filename, content) in &files_and_content {
        let file_path = temp_dir.path().join(filename);
        fs::write(&file_path, content).await.unwrap();
        
        let cas_id = CasGenerator::generate_cas_id(&file_path).await.unwrap();
        cas_ids.push(cas_id);
    }
    
    // file1 and file4 should have same CAS ID (same content)
    assert_eq!(cas_ids[0], cas_ids[3]);
    
    // All others should be different
    assert_ne!(cas_ids[0], cas_ids[1]);
    assert_ne!(cas_ids[0], cas_ids[2]);
    assert_ne!(cas_ids[1], cas_ids[2]);
}

#[tokio::test]
async fn test_cas_binary_files() {
    let temp_dir = TempDir::new().unwrap();
    
    // Test with binary content
    let binary_content = vec![0u8, 1, 2, 255, 128, 64, 32, 16, 8, 4, 2, 1];
    let file_path = temp_dir.path().join("binary_file.bin");
    fs::write(&file_path, &binary_content).await.unwrap();
    
    let cas_id = CasGenerator::generate_cas_id(&file_path).await.unwrap();
    assert!(cas_id.starts_with(&format!("v{}_full:", CURRENT_CAS_VERSION)));
    
    // Should be reproducible
    let cas_id2 = CasGenerator::generate_cas_id(&file_path).await.unwrap();
    assert_eq!(cas_id, cas_id2);
}

#[tokio::test]
async fn test_cas_threshold_boundary() {
    let temp_dir = TempDir::new().unwrap();
    
    // Test file exactly at threshold
    let file_path = temp_dir.path().join("threshold_file.dat");
    let content = vec![b'X'; SMALL_FILE_THRESHOLD as usize];
    fs::write(&file_path, &content).await.unwrap();
    
    let cas_id = CasGenerator::generate_cas_id(&file_path).await.unwrap();
    // At threshold should still use full hash
    assert!(cas_id.starts_with(&format!("v{}_full:", CURRENT_CAS_VERSION)));
    
    // Test file just over threshold
    let file_path2 = temp_dir.path().join("over_threshold_file.dat");
    let mut content2 = content;
    content2.push(b'Y'); // One byte over
    fs::write(&file_path2, &content2).await.unwrap();
    
    let cas_id2 = CasGenerator::generate_cas_id(&file_path2).await.unwrap();
    // Over threshold should use sampling
    assert!(cas_id2.starts_with(&format!("v{}_sampled:", CURRENT_CAS_VERSION)));
}

#[tokio::test]
async fn test_content_identity_creation() {
    let cas_id = "v2_full:abc123def456";
    let content = ContentIdentity::new(cas_id.to_string(), CURRENT_CAS_VERSION);
    
    assert_eq!(content.cas_id, cas_id);
    assert_eq!(content.cas_version, CURRENT_CAS_VERSION);
    assert_eq!(content.entry_count, 1);
    assert_eq!(content.kind, ContentKind::Unknown);
    assert!(content.full_hash.is_none());
    assert!(content.mime_type.is_none());
    assert_eq!(content.total_size, 0);
    assert!(!content.is_orphaned());
}

#[tokio::test]
async fn test_content_identity_operations() {
    let mut content = ContentIdentity::new("v2_full:test".to_string(), CURRENT_CAS_VERSION);
    
    // Test entry count operations
    content.increment_entry_count();
    assert_eq!(content.entry_count, 2);
    
    content.increment_entry_count();
    assert_eq!(content.entry_count, 3);
    
    content.decrement_entry_count();
    assert_eq!(content.entry_count, 2);
    
    // Test size operations
    content.add_size(1024);
    assert_eq!(content.total_size, 1024);
    
    content.add_size(512);
    assert_eq!(content.total_size, 1536);
    
    content.remove_size(256);
    assert_eq!(content.total_size, 1280);
    
    // Test saturating subtraction
    content.remove_size(2000);
    assert_eq!(content.total_size, 0);
    
    // Test orphaned detection
    content.decrement_entry_count();
    content.decrement_entry_count();
    assert_eq!(content.entry_count, 0);
    assert!(content.is_orphaned());
    
    // Can't go below zero
    content.decrement_entry_count();
    assert_eq!(content.entry_count, 0);
}

#[tokio::test]
async fn test_content_identity_mime_type_detection() {
    let mut content = ContentIdentity::new("v2_full:test".to_string(), CURRENT_CAS_VERSION);
    
    // Test MIME type detection
    content.set_mime_type("image/jpeg".to_string());
    assert_eq!(content.kind, ContentKind::Image);
    assert_eq!(content.mime_type, Some("image/jpeg".to_string()));
    
    content.set_mime_type("video/mp4".to_string());
    assert_eq!(content.kind, ContentKind::Video);
    
    content.set_mime_type("audio/wav".to_string());
    assert_eq!(content.kind, ContentKind::Audio);
    
    content.set_mime_type("text/plain".to_string());
    assert_eq!(content.kind, ContentKind::Text);
    
    content.set_mime_type("application/pdf".to_string());
    assert_eq!(content.kind, ContentKind::Document);
    
    content.set_mime_type("application/zip".to_string());
    assert_eq!(content.kind, ContentKind::Archive);
    
    content.set_mime_type("application/unknown".to_string());
    assert_eq!(content.kind, ContentKind::Unknown);
}

#[tokio::test]
async fn test_content_kind_enum() {
    // Test that all ContentKind variants are properly defined
    let kinds = vec![
        ContentKind::Image,
        ContentKind::Video,
        ContentKind::Audio,
        ContentKind::Document,
        ContentKind::Archive,
        ContentKind::Code,
        ContentKind::Text,
        ContentKind::Database,
        ContentKind::Book,
        ContentKind::Font,
        ContentKind::Mesh,
        ContentKind::Config,
        ContentKind::Encrypted,
        ContentKind::Key,
        ContentKind::Executable,
        ContentKind::Binary,
        ContentKind::Unknown,
    ];
    
    // Test serialization/deserialization
    for kind in kinds {
        let serialized = serde_json::to_string(&kind).unwrap();
        let deserialized: ContentKind = serde_json::from_str(&serialized).unwrap();
        assert_eq!(kind, deserialized);
    }
}

#[tokio::test]
async fn test_media_data_structures() {
    let gps = GpsCoordinates {
        latitude: 37.7749,
        longitude: -122.4194,
        altitude: Some(100.0),
    };
    
    let exif = ExifData {
        make: Some("Canon".to_string()),
        model: Some("EOS R5".to_string()),
        date_taken: Some(chrono::Utc::now()),
        gps: Some(gps),
        iso: Some(400),
        aperture: Some(2.8),
        shutter_speed: Some(0.005),
        focal_length: Some(85.0),
    };
    
    let media_data = MediaData {
        width: Some(1920),
        height: Some(1080),
        duration: Some(120.5),
        bitrate: Some(5000000),
        fps: Some(29.97),
        exif: Some(exif),
        extra: serde_json::json!({
            "color_space": "sRGB",
            "compression": "JPEG"
        }),
    };
    
    // Test serialization
    let serialized = serde_json::to_string(&media_data).unwrap();
    let deserialized: MediaData = serde_json::from_str(&serialized).unwrap();
    
    assert_eq!(media_data.width, deserialized.width);
    assert_eq!(media_data.height, deserialized.height);
    assert_eq!(media_data.duration, deserialized.duration);
    assert!(deserialized.exif.is_some());
    
    let deserialized_exif = deserialized.exif.unwrap();
    assert_eq!(media_data.exif.as_ref().unwrap().make, deserialized_exif.make);
    assert_eq!(media_data.exif.as_ref().unwrap().iso, deserialized_exif.iso);
}

#[tokio::test]
async fn test_cas_version_constant() {
    // Test that the version constant is properly defined
    assert!(CURRENT_CAS_VERSION > 0);
    assert!(CURRENT_CAS_VERSION < 100); // Reasonable upper bound
    
    // Test that small file threshold is reasonable
    assert!(SMALL_FILE_THRESHOLD > 1024); // At least 1KB
    assert!(SMALL_FILE_THRESHOLD < 1024 * 1024 * 1024); // Less than 1GB
}

#[tokio::test]
async fn test_cas_concurrent_generation() {
    let temp_dir = TempDir::new().unwrap();
    
    // Create multiple files
    let mut handles = Vec::new();
    
    for i in 0..10 {
        let file_path = temp_dir.path().join(format!("concurrent_file_{}.txt", i));
        let content = format!("Content for file {}", i);
        fs::write(&file_path, content.as_bytes()).await.unwrap();
        
        // Generate CAS IDs concurrently
        let handle = tokio::spawn(async move {
            CasGenerator::generate_cas_id(&file_path).await
        });
        
        handles.push(handle);
    }
    
    // Wait for all to complete
    let results: Vec<_> = futures::future::join_all(handles).await;
    
    // All should succeed
    for result in results {
        let cas_id = result.unwrap().unwrap();
        assert!(cas_id.starts_with(&format!("v{}_", CURRENT_CAS_VERSION)));
    }
}