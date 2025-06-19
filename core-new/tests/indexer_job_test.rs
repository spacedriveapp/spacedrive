//! Integration tests for the indexer job

use sd_core_new::{
    infrastructure::jobs::{
        manager::JobManager,
        traits::{Job, JobHandler},
        context::JobContext,
        error::JobResult,
    },
    operations::indexing::indexer_job::{
        IndexerJob, IndexMode, IndexerProgress, IndexerStats, IndexPhase, IndexerOutput,
    },
    shared::types::SdPath,
    domain::content_identity::CasGenerator,
};
use std::{
    collections::HashMap,
    path::PathBuf,
    time::Duration,
};
use tempfile::TempDir;
use tokio::fs;
use uuid::Uuid;

/// Helper function to create a test directory structure
async fn create_test_directory_structure(base_path: &std::path::Path) -> std::io::Result<()> {
    // Create directory structure
    fs::create_dir_all(base_path.join("documents")).await?;
    fs::create_dir_all(base_path.join("images/photos")).await?;
    fs::create_dir_all(base_path.join("images/graphics")).await?;
    fs::create_dir_all(base_path.join("videos")).await?;
    fs::create_dir_all(base_path.join("empty_dir")).await?;
    
    // Create files
    fs::write(base_path.join("readme.txt"), "This is a readme file").await?;
    fs::write(base_path.join("documents/report.pdf"), "PDF content").await?;
    fs::write(base_path.join("documents/notes.txt"), "Some notes here").await?;
    fs::write(base_path.join("images/photos/vacation.jpg"), "JPEG image data").await?;
    fs::write(base_path.join("images/photos/family.png"), "PNG image data").await?;
    fs::write(base_path.join("images/graphics/logo.svg"), "SVG vector data").await?;
    fs::write(base_path.join("videos/movie.mp4"), "MP4 video data").await?;
    
    // Create some larger files for testing
    let large_content = "X".repeat(1024); // 1KB
    fs::write(base_path.join("large_file.dat"), large_content).await?;
    
    // Create a hidden file
    fs::write(base_path.join(".hidden_file"), "Hidden content").await?;
    
    Ok(())
}

#[tokio::test]
async fn test_indexer_job_creation() {
    let location_id = Uuid::new_v4();
    let device_id = Uuid::new_v4();
    let root_path = SdPath::new(device_id, PathBuf::from("/test/path"));
    
    // Test different indexing modes
    let shallow_job = IndexerJob::new(location_id, root_path.clone(), IndexMode::Shallow);
    let content_job = IndexerJob::new(location_id, root_path.clone(), IndexMode::Content);
    let deep_job = IndexerJob::new(location_id, root_path.clone(), IndexMode::Deep);
    
    assert_eq!(shallow_job.location_id, location_id);
    assert_eq!(content_job.location_id, location_id);
    assert_eq!(deep_job.location_id, location_id);
    
    // Test job constants
    assert_eq!(IndexerJob::NAME, "indexer");
    assert_eq!(IndexerJob::RESUMABLE, true);
    assert!(IndexerJob::DESCRIPTION.is_some());
}

#[tokio::test]
async fn test_indexer_job_serialization() {
    let location_id = Uuid::new_v4();
    let device_id = Uuid::new_v4();
    let root_path = SdPath::new(device_id, PathBuf::from("/test/indexing"));
    
    let original_job = IndexerJob::new(location_id, root_path, IndexMode::Deep);
    
    // Test serialization/deserialization
    let serialized = rmp_serde::to_vec(&original_job).unwrap();
    let deserialized: IndexerJob = rmp_serde::from_slice(&serialized).unwrap();
    
    assert_eq!(original_job.location_id, deserialized.location_id);
    assert_eq!(original_job.root_path, deserialized.root_path);
    assert_eq!(original_job.mode, deserialized.mode);
}

#[tokio::test]
async fn test_index_mode_ordering() {
    // Test that IndexMode has proper ordering for comparisons
    assert!(IndexMode::Shallow < IndexMode::Content);
    assert!(IndexMode::Content < IndexMode::Deep);
    
    // Test that we can use >= for mode checks like in the job
    assert!(IndexMode::Content >= IndexMode::Content);
    assert!(IndexMode::Deep >= IndexMode::Content);
    assert!(IndexMode::Deep >= IndexMode::Shallow);
    assert!(!(IndexMode::Shallow >= IndexMode::Content));
}

#[tokio::test]
async fn test_indexer_progress_structures() {
    // Test IndexPhase variants
    let discovery_phase = IndexPhase::Discovery { dirs_queued: 5 };
    let processing_phase = IndexPhase::Processing { batch: 3, total_batches: 10 };
    let content_phase = IndexPhase::ContentIdentification { current: 50, total: 100 };
    let finalizing_phase = IndexPhase::Finalizing;
    
    // Test IndexerProgress
    let progress = IndexerProgress {
        phase: discovery_phase,
        current_path: "/test/path".to_string(),
        total_found: IndexerStats {
            files: 100,
            dirs: 20,
            bytes: 1024 * 1024,
            symlinks: 5,
        },
        processing_rate: 15.5,
        estimated_remaining: Some(Duration::from_secs(120)),
    };
    
    // Test serialization
    let serialized = serde_json::to_string(&progress).unwrap();
    let deserialized: IndexerProgress = serde_json::from_str(&serialized).unwrap();
    
    assert_eq!(progress.current_path, deserialized.current_path);
    assert_eq!(progress.total_found.files, deserialized.total_found.files);
    assert_eq!(progress.processing_rate, deserialized.processing_rate);
}

#[tokio::test]
async fn test_indexer_stats_operations() {
    let mut stats = IndexerStats::default();
    
    assert_eq!(stats.files, 0);
    assert_eq!(stats.dirs, 0);
    assert_eq!(stats.bytes, 0);
    assert_eq!(stats.symlinks, 0);
    
    // Test manual updates
    stats.files += 10;
    stats.dirs += 2;
    stats.bytes += 1024;
    stats.symlinks += 1;
    
    assert_eq!(stats.files, 10);
    assert_eq!(stats.dirs, 2);
    assert_eq!(stats.bytes, 1024);
    assert_eq!(stats.symlinks, 1);
}

#[tokio::test]
async fn test_indexer_output_conversion() {
    let output = IndexerOutput {
        location_id: Uuid::new_v4(),
        stats: IndexerStats {
            files: 150,
            dirs: 25,
            bytes: 5 * 1024 * 1024,
            symlinks: 3,
        },
        duration: Duration::from_secs(45),
        errors: Vec::new(), // This will be empty for the test
    };
    
    // Test conversion to JobOutput
    let job_output = output.into();
    match job_output {
        sd_core_new::infrastructure::jobs::output::JobOutput::Indexed { 
            total_files, 
            total_dirs, 
            total_bytes 
        } => {
            assert_eq!(total_files, 150);
            assert_eq!(total_dirs, 25);
            assert_eq!(total_bytes, 5 * 1024 * 1024);
        }
        _ => panic!("Expected Indexed job output"),
    }
}

#[tokio::test]
async fn test_cas_integration_with_test_files() {
    let temp_dir = TempDir::new().unwrap();
    let test_dir = temp_dir.path().join("cas_test");
    
    // Create test files
    fs::create_dir_all(&test_dir).await.unwrap();
    fs::write(test_dir.join("text_file.txt"), "Hello CAS world!").await.unwrap();
    fs::write(test_dir.join("binary_file.bin"), &[0, 1, 2, 255, 128]).await.unwrap();
    
    // Test CAS generation for different file types
    let text_cas = CasGenerator::generate_cas_id(&test_dir.join("text_file.txt")).await.unwrap();
    let binary_cas = CasGenerator::generate_cas_id(&test_dir.join("binary_file.bin")).await.unwrap();
    
    assert_ne!(text_cas, binary_cas);
    assert!(text_cas.starts_with("v2_full:"));
    assert!(binary_cas.starts_with("v2_full:"));
    
    // Test verification
    assert!(CasGenerator::verify_cas_id(&test_dir.join("text_file.txt"), &text_cas).await.unwrap());
    assert!(CasGenerator::verify_cas_id(&test_dir.join("binary_file.bin"), &binary_cas).await.unwrap());
    
    // Cross-verification should fail
    assert!(!CasGenerator::verify_cas_id(&test_dir.join("text_file.txt"), &binary_cas).await.unwrap());
    assert!(!CasGenerator::verify_cas_id(&test_dir.join("binary_file.bin"), &text_cas).await.unwrap());
}

#[tokio::test]
async fn test_directory_traversal_logic() {
    let temp_dir = TempDir::new().unwrap();
    let test_dir = temp_dir.path().join("traversal_test");
    
    // Create a complex directory structure
    create_test_directory_structure(&test_dir).await.unwrap();
    
    // Test that we can read directories (simplified test of the logic used in indexer)
    let mut total_files = 0;
    let mut total_dirs = 0;
    
    // Use a simple iterative approach instead of recursion to avoid boxing
    let mut dirs_to_process = vec![test_dir.to_path_buf()];
    
    while let Some(current_dir) = dirs_to_process.pop() {
        if let Ok(mut entries) = fs::read_dir(&current_dir).await {
            while let Some(entry) = entries.next_entry().await.unwrap_or(None) {
                if let Ok(metadata) = entry.metadata().await {
                    if metadata.is_dir() {
                        total_dirs += 1;
                        dirs_to_process.push(entry.path());
                    } else if metadata.is_file() {
                        total_files += 1;
                    }
                }
            }
        }
    }
    
    // Should find the files and directories we created
    assert!(total_files >= 8); // At least the files we created
    assert!(total_dirs >= 5);  // At least the directories we created
}

#[tokio::test]
async fn test_large_file_handling() {
    let temp_dir = TempDir::new().unwrap();
    let large_file = temp_dir.path().join("large_test_file.dat");
    
    // Create a file larger than the CAS threshold
    let chunk = vec![b'L'; 1024 * 1024]; // 1MB chunk
    let mut file = std::fs::File::create(&large_file).unwrap();
    use std::io::Write;
    
    // Write 15MB (over the 10MB threshold)
    for i in 0..15 {
        let mut varied_chunk = chunk.clone();
        varied_chunk[0] = (i % 256) as u8; // Vary content slightly
        file.write_all(&varied_chunk).unwrap();
    }
    drop(file);
    
    // Test CAS generation for large file
    let cas_id = CasGenerator::generate_cas_id(&large_file).await.unwrap();
    assert!(cas_id.starts_with("v2_sampled:"));
    
    // Should be reproducible
    let cas_id2 = CasGenerator::generate_cas_id(&large_file).await.unwrap();
    assert_eq!(cas_id, cas_id2);
    
    // Test verification
    assert!(CasGenerator::verify_cas_id(&large_file, &cas_id).await.unwrap());
}

#[tokio::test]
async fn test_symlink_handling() {
    let temp_dir = TempDir::new().unwrap();
    let target_file = temp_dir.path().join("target.txt");
    let symlink_file = temp_dir.path().join("link.txt");
    
    // Create target file
    fs::write(&target_file, "Target content").await.unwrap();
    
    // Create symlink (skip on Windows where this might not be available)
    #[cfg(unix)]
    {
        tokio::process::Command::new("ln")
            .args(&["-s", "target.txt", "link.txt"])
            .current_dir(temp_dir.path())
            .output()
            .await
            .unwrap();
        
        // Test reading the symlink metadata
        let metadata = fs::symlink_metadata(&symlink_file).await.unwrap();
        assert!(metadata.is_symlink());
        
        // The indexer should detect this as a symlink
        let file_type = metadata.file_type();
        assert!(file_type.is_symlink());
    }
}

#[tokio::test]
async fn test_error_handling_scenarios() {
    let temp_dir = TempDir::new().unwrap();
    
    // Test with non-existent path
    let non_existent = temp_dir.path().join("does_not_exist");
    let result = fs::read_dir(&non_existent).await;
    assert!(result.is_err());
    
    // Test with permission denied scenario (create unreadable directory on Unix)
    #[cfg(unix)]
    {
        let restricted_dir = temp_dir.path().join("restricted");
        fs::create_dir(&restricted_dir).await.unwrap();
        
        // Remove read permissions
        let mut perms = fs::metadata(&restricted_dir).await.unwrap().permissions();
        use std::os::unix::fs::PermissionsExt;
        perms.set_mode(0o000); // No permissions
        fs::set_permissions(&restricted_dir, perms).await.unwrap();
        
        // Reading should fail
        let result = fs::read_dir(&restricted_dir).await;
        assert!(result.is_err());
        
        // Restore permissions for cleanup
        let mut perms = fs::metadata(&restricted_dir).await.unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&restricted_dir, perms).await.unwrap();
    }
}

#[tokio::test]
async fn test_file_metadata_extraction() {
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("metadata_test.txt");
    
    let content = "Test content for metadata extraction";
    fs::write(&test_file, content).await.unwrap();
    
    // Test metadata reading (similar to what indexer does)
    let metadata = fs::metadata(&test_file).await.unwrap();
    
    assert!(metadata.is_file());
    assert!(!metadata.is_dir());
    assert!(!metadata.is_symlink());
    assert_eq!(metadata.len() as usize, content.len());
    
    // Test modified time
    let modified = metadata.modified().unwrap();
    let now = std::time::SystemTime::now();
    let duration_since_modified = now.duration_since(modified).unwrap();
    assert!(duration_since_modified < Duration::from_secs(10)); // Should be recent
}

#[tokio::test]
async fn test_batch_processing_logic() {
    // Test the batching logic used in the indexer
    let mut items = Vec::new();
    let batch_size = 1000;
    
    // Add items one by one and test batching
    for i in 0..2500 {
        items.push(format!("item_{}", i));
        
        // When we reach batch size, we should process
        if items.len() >= batch_size {
            let batch = std::mem::take(&mut items);
            assert_eq!(batch.len(), batch_size);
            
            // Simulate processing batch (in real indexer, this would be database operations)
            assert!(!batch.is_empty());
        }
    }
    
    // Handle remaining items
    if !items.is_empty() {
        let final_batch = std::mem::take(&mut items);
        assert_eq!(final_batch.len(), 500); // 2500 - 2*1000 = 500
    }
}

#[tokio::test]
async fn test_concurrent_cas_generation() {
    let temp_dir = TempDir::new().unwrap();
    
    // Create multiple files
    let mut files = Vec::new();
    for i in 0..20 {
        let file_path = temp_dir.path().join(format!("concurrent_file_{}.txt", i));
        let content = format!("Content for concurrent test file {}", i);
        fs::write(&file_path, content).await.unwrap();
        files.push(file_path);
    }
    
    // Generate CAS IDs concurrently
    let tasks: Vec<_> = files.into_iter().map(|file_path| {
        tokio::spawn(async move {
            CasGenerator::generate_cas_id(&file_path).await
        })
    }).collect();
    
    // Wait for all to complete
    let results = futures::future::join_all(tasks).await;
    
    // All should succeed
    let mut cas_ids = Vec::new();
    for result in results {
        let cas_id = result.unwrap().unwrap();
        assert!(cas_id.starts_with("v2_full:"));
        cas_ids.push(cas_id);
    }
    
    // All CAS IDs should be different (different content)
    let unique_count = cas_ids.iter().collect::<std::collections::HashSet<_>>().len();
    assert_eq!(unique_count, cas_ids.len());
}

#[tokio::test]
async fn test_indexer_job_schema() {
    let schema = IndexerJob::schema();
    
    assert_eq!(schema.name, "indexer");
    assert_eq!(schema.version, 1);
    assert!(schema.resumable);
    assert!(schema.description.is_some());
}

#[tokio::test]
async fn test_performance_tracking_structures() {
    use std::time::Instant;
    
    // Test the performance tracking logic similar to what's in IndexerState
    let start_time = Instant::now();
    let mut items_processed = 0u64;
    let mut last_update = Instant::now();
    
    // Simulate processing items
    for _ in 0..1000 {
        items_processed += 1;
        
        // Every 100 items, calculate rate
        if items_processed % 100 == 0 {
            let elapsed = last_update.elapsed();
            if elapsed.as_secs() > 0 {
                let rate = 100.0 / elapsed.as_secs_f32();
                assert!(rate > 0.0);
                last_update = Instant::now();
            }
        }
    }
    
    let total_duration = start_time.elapsed();
    assert!(total_duration.as_nanos() > 0);
}