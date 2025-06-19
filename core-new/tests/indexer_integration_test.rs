//! Comprehensive indexer job integration tests

use sd_core_new::{
    operations::indexing::indexer_job::{IndexerJob, IndexMode, IndexerOutput, IndexError},
    shared::types::{SdPath, set_current_device_id},
    infrastructure::jobs::{
        traits::Job,
        context::CheckpointHandler,
        error::JobResult,
        types::JobId,
    },
};
use tempfile::TempDir;
use tokio::fs;
use uuid::Uuid;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use serde_json::Value;

/// Helper to create test files with content
async fn create_test_file(path: &std::path::Path, content: &str) -> Result<(), std::io::Error> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await?;
    }
    fs::write(path, content).await
}

/// Mock checkpoint handler for testing
#[derive(Debug, Default)]
struct MockCheckpointHandler {
    checkpoints: Arc<Mutex<HashMap<JobId, Option<Vec<u8>>>>>,
}

#[async_trait::async_trait]
impl CheckpointHandler for MockCheckpointHandler {
    async fn save_checkpoint(&self, job_id: JobId, data: Option<Vec<u8>>) -> JobResult<()> {
        let mut checkpoints = self.checkpoints.lock().unwrap();
        checkpoints.insert(job_id, data);
        Ok(())
    }
    
    async fn load_checkpoint(&self, job_id: JobId) -> JobResult<Option<Vec<u8>>> {
        let checkpoints = self.checkpoints.lock().unwrap();
        Ok(checkpoints.get(&job_id).cloned().unwrap_or(None))
    }
    
    async fn delete_checkpoint(&self, job_id: JobId) -> JobResult<()> {
        let mut checkpoints = self.checkpoints.lock().unwrap();
        checkpoints.remove(&job_id);
        Ok(())
    }
}

// Since IndexerJob tests don't need actual job execution, 
// we focus on testing job creation, serialization, and configuration

#[tokio::test]
async fn test_indexer_job_creation() {
    let device_id = Uuid::new_v4();
    let location_id = Uuid::new_v4();
    set_current_device_id(device_id);
    
    let temp_dir = TempDir::new().unwrap();
    let root_path = SdPath::new(device_id, temp_dir.path().to_path_buf());
    
    // Test shallow indexing
    let shallow_job = IndexerJob::new(location_id, root_path.clone(), IndexMode::Shallow);
    assert_eq!(shallow_job.location_id, location_id);
    assert_eq!(shallow_job.mode, IndexMode::Shallow);
    
    // Test content indexing
    let content_job = IndexerJob::new(location_id, root_path.clone(), IndexMode::Content);
    assert_eq!(content_job.mode, IndexMode::Content);
    
    // Test deep indexing
    let deep_job = IndexerJob::new(location_id, root_path, IndexMode::Deep);
    assert_eq!(deep_job.mode, IndexMode::Deep);
    
    println!("✅ Indexer job creation test passed");
}

#[tokio::test]
async fn test_indexer_job_constants() {
    // Test job constants
    assert_eq!(IndexerJob::NAME, "indexer");
    assert_eq!(IndexerJob::RESUMABLE, true);
    assert!(IndexerJob::DESCRIPTION.is_some());
    
    // Test index mode ordering
    assert!(IndexMode::Shallow < IndexMode::Content);
    assert!(IndexMode::Content < IndexMode::Deep);
    
    println!("✅ Indexer job constants test passed");
}

#[tokio::test]
async fn test_indexer_mode_comparison() {
    // Test that modes can be compared for feature inclusion
    assert!(IndexMode::Content >= IndexMode::Shallow);
    assert!(IndexMode::Deep >= IndexMode::Content);
    assert!(IndexMode::Deep >= IndexMode::Shallow);
    
    // Test specific comparisons used in indexer logic
    let content_mode = IndexMode::Content;
    let shallow_mode = IndexMode::Shallow;
    let deep_mode = IndexMode::Deep;
    
    assert!(content_mode >= IndexMode::Content);
    assert!(deep_mode >= IndexMode::Content);
    assert!(!(shallow_mode >= IndexMode::Content));
    
    println!("✅ Indexer mode comparison test passed");
}

#[tokio::test]
async fn test_indexer_with_empty_directory() {
    let device_id = Uuid::new_v4();
    let location_id = Uuid::new_v4();
    set_current_device_id(device_id);
    
    let temp_dir = TempDir::new().unwrap();
    let empty_dir = temp_dir.path().join("empty");
    fs::create_dir_all(&empty_dir).await.unwrap();
    
    let root_path = SdPath::new(device_id, empty_dir);
    let indexer_job = IndexerJob::new(location_id, root_path, IndexMode::Shallow);
    
    // Verify job setup for empty directory
    assert_eq!(indexer_job.location_id, location_id);
    assert_eq!(indexer_job.mode, IndexMode::Shallow);
    
    println!("✅ Empty directory indexer test passed");
}

#[tokio::test]
async fn test_indexer_with_simple_files() {
    let device_id = Uuid::new_v4();
    let location_id = Uuid::new_v4();
    set_current_device_id(device_id);
    
    let temp_dir = TempDir::new().unwrap();
    let test_dir = temp_dir.path().join("test_files");
    
    // Create test files
    let test_files = [
        ("file1.txt", "Simple text content"),
        ("file2.md", "# Markdown content\n\nSome text here."),
        ("data.json", r#"{"key": "value", "number": 42}"#),
    ];
    
    for (filename, content) in &test_files {
        let file_path = test_dir.join(filename);
        create_test_file(&file_path, content).await.unwrap();
    }
    
    let root_path = SdPath::new(device_id, test_dir);
    let indexer_job = IndexerJob::new(location_id, root_path, IndexMode::Content);
    
    // Verify job is set up correctly for content indexing
    assert_eq!(indexer_job.location_id, location_id);
    assert_eq!(indexer_job.mode, IndexMode::Content);
    assert!(indexer_job.mode >= IndexMode::Content); // Should generate content IDs
    
    println!("✅ Simple files indexer test passed");
}

#[tokio::test]
async fn test_indexer_with_directory_structure() {
    let device_id = Uuid::new_v4();
    let location_id = Uuid::new_v4();
    set_current_device_id(device_id);
    
    let temp_dir = TempDir::new().unwrap();
    let test_dir = temp_dir.path().join("complex_structure");
    
    // Create complex directory structure
    let test_structure = [
        ("root.txt", "Root level file"),
        ("docs/readme.md", "# Documentation\n\nProject documentation here."),
        ("docs/api/endpoints.json", r#"{"endpoints": ["/api/v1", "/api/v2"]}"#),
        ("src/main.rs", "fn main() { println!(\"Hello, world!\"); }"),
        ("src/lib/utils.rs", "pub fn helper() -> String { \"utility\".to_string() }"),
        ("tests/integration.rs", "#[test] fn test_example() { assert!(true); }"),
        ("assets/config.toml", "[app]\nname = \"test\"\nversion = \"1.0.0\""),
    ];
    
    for (filename, content) in &test_structure {
        let file_path = test_dir.join(filename);
        create_test_file(&file_path, content).await.unwrap();
    }
    
    let root_path = SdPath::new(device_id, test_dir);
    let indexer_job = IndexerJob::new(location_id, root_path, IndexMode::Deep);
    
    // Verify job setup for deep indexing
    assert_eq!(indexer_job.location_id, location_id);
    assert_eq!(indexer_job.mode, IndexMode::Deep);
    
    // Verify all test files were created
    for (filename, _) in &test_structure {
        let file_path = temp_dir.path().join("complex_structure").join(filename);
        assert!(file_path.exists(), "Test file {} was not created", filename);
    }
    
    println!("✅ Directory structure indexer test passed");
}

#[tokio::test]
async fn test_indexer_job_serialization() {
    let device_id = Uuid::new_v4();
    let location_id = Uuid::new_v4();
    set_current_device_id(device_id);
    
    let temp_dir = TempDir::new().unwrap();
    let root_path = SdPath::new(device_id, temp_dir.path().to_path_buf());
    
    // Test serialization of all index modes
    let modes = [IndexMode::Shallow, IndexMode::Content, IndexMode::Deep];
    
    for mode in &modes {
        let indexer_job = IndexerJob::new(location_id, root_path.clone(), *mode);
        
        // Test with JSON serialization
        let json_serialized = serde_json::to_string(&indexer_job).unwrap();
        let json_deserialized: IndexerJob = serde_json::from_str(&json_serialized).unwrap();
        
        // Verify key fields are preserved
        assert_eq!(indexer_job.location_id, json_deserialized.location_id);
        assert_eq!(indexer_job.mode, json_deserialized.mode);
        assert_eq!(indexer_job.root_path.device_id, json_deserialized.root_path.device_id);
    }
    
    println!("✅ Indexer job serialization test passed");
}

#[tokio::test]
async fn test_indexer_error_types() {
    // Test IndexError variants
    let read_error = IndexError::ReadDir {
        path: "/test/path".to_string(),
        error: "Permission denied".to_string(),
    };
    
    match read_error {
        IndexError::ReadDir { path, error } => {
            assert_eq!(path, "/test/path");
            assert_eq!(error, "Permission denied");
        }
        _ => panic!("Expected ReadDir error"),
    }
    
    let create_error = IndexError::CreateEntry {
        path: "/test/file.txt".to_string(),
        error: "Database error".to_string(),
    };
    
    match create_error {
        IndexError::CreateEntry { path, error } => {
            assert_eq!(path, "/test/file.txt");
            assert_eq!(error, "Database error");
        }
        _ => panic!("Expected CreateEntry error"),
    }
    
    let content_error = IndexError::ContentId {
        path: "/test/content.txt".to_string(),
        error: "CAS generation failed".to_string(),
    };
    
    match content_error {
        IndexError::ContentId { path, error } => {
            assert_eq!(path, "/test/content.txt");
            assert_eq!(error, "CAS generation failed");
        }
        _ => panic!("Expected ContentId error"),
    }
    
    println!("✅ Indexer error types test passed");
}

#[tokio::test]
async fn test_indexer_output_conversion() {
    let location_id = Uuid::new_v4();
    
    // Test IndexerOutput creation and conversion
    let output = IndexerOutput {
        location_id,
        stats: sd_core_new::operations::indexing::indexer_job::IndexerStats {
            files: 150,
            dirs: 25,
            bytes: 1024 * 1024 * 10, // 10MB
            symlinks: 5,
        },
        duration: Duration::from_secs(45),
        errors: vec![],
    };
    
    // Test conversion to JobOutput
    let job_output: sd_core_new::infrastructure::jobs::output::JobOutput = output.into();
    
    match job_output {
        sd_core_new::infrastructure::jobs::output::JobOutput::Indexed { 
            total_files, 
            total_dirs, 
            total_bytes 
        } => {
            assert_eq!(total_files, 150);
            assert_eq!(total_dirs, 25);
            assert_eq!(total_bytes, 1024 * 1024 * 10);
        }
        _ => panic!("Expected Indexed output"),
    }
    
    println!("✅ Indexer output conversion test passed");
}

#[tokio::test]
async fn test_indexer_with_mixed_file_types() {
    let device_id = Uuid::new_v4();
    let location_id = Uuid::new_v4();
    set_current_device_id(device_id);
    
    let temp_dir = TempDir::new().unwrap();
    let test_dir = temp_dir.path().join("mixed_files");
    
    // Create files of different types and sizes
    let mixed_files = [
        ("small.txt", "Small file"),
        ("medium.json", &"x".repeat(1000)), // 1KB file
        ("large.log", &"log entry\n".repeat(1000)), // ~10KB file
        ("binary.dat", &[0u8; 512].iter().map(|_| "A").collect::<String>()), // Binary-like content
        ("empty.txt", ""), // Empty file
        ("unicode.txt", "Hello 世界 🌍 Ñoël"), // Unicode content
    ];
    
    for (filename, content) in &mixed_files {
        let file_path = test_dir.join(filename);
        create_test_file(&file_path, content).await.unwrap();
    }
    
    // Create some subdirectories
    let subdirs = ["subdir1", "subdir2/nested", "empty_dir"];
    for subdir in &subdirs {
        let dir_path = test_dir.join(subdir);
        fs::create_dir_all(&dir_path).await.unwrap();
    }
    
    // Add files in subdirectories
    let subdir_files = [
        ("subdir1/nested_file.rs", "fn test() { println!(\"nested\"); }"),
        ("subdir2/nested/deep_file.md", "# Deep nested file"),
    ];
    
    for (filename, content) in &subdir_files {
        let file_path = test_dir.join(filename);
        create_test_file(&file_path, content).await.unwrap();
    }
    
    let root_path = SdPath::new(device_id, test_dir);
    let indexer_job = IndexerJob::new(location_id, root_path, IndexMode::Content);
    
    // Verify job setup
    assert_eq!(indexer_job.location_id, location_id);
    assert_eq!(indexer_job.mode, IndexMode::Content);
    
    // Verify all files exist
    let total_files = mixed_files.len() + subdir_files.len();
    let mut found_files = 0;
    
    for (filename, _) in mixed_files.iter().chain(subdir_files.iter()) {
        let file_path = temp_dir.path().join("mixed_files").join(filename);
        if file_path.exists() {
            found_files += 1;
        }
    }
    
    assert_eq!(found_files, total_files, "Not all test files were created");
    
    println!("✅ Mixed file types indexer test passed");
}

#[tokio::test]
async fn test_indexer_job_schema() {
    // Test job schema information
    let schema = IndexerJob::schema();
    
    assert_eq!(schema.name, "indexer");
    assert_eq!(schema.version, 1);
    assert!(schema.resumable);
    
    // Test that schema contains expected field information
    let serialized_schema = serde_json::to_value(&schema).unwrap();
    
    match serialized_schema {
        Value::Object(obj) => {
            assert!(obj.contains_key("name"));
            assert!(obj.contains_key("version"));
            assert!(obj.contains_key("resumable"));
        }
        _ => panic!("Expected schema object"),
    }
    
    println!("✅ Indexer job schema test passed");
}