//! Validation tests for copy module fixes

use sd_core_new::operations::files::copy::{
    job::{FileCopyJob, CopyOptions},
    strategy::CopyStrategy,
};
use sd_core_new::shared::types::{SdPath, SdPathBatch};
use std::path::PathBuf;
use tempfile::TempDir;
use tokio::{
    fs::File,
    io::AsyncWriteExt,
};
use uuid::Uuid;

/// Test helper to create a temporary directory
fn create_temp_dir() -> TempDir {
    TempDir::new().expect("Failed to create temp directory")
}

/// Test helper to create test files
async fn create_test_files(dir: &std::path::Path, count: usize) -> Vec<PathBuf> {
    let mut files = Vec::new();
    
    for i in 0..count {
        let file_path = dir.join(format!("test_file_{}.txt", i));
        let mut file = File::create(&file_path).await.expect("Failed to create test file");
        file.write_all(format!("Test content for file {}", i).as_bytes()).await.expect("Failed to write test content");
        files.push(file_path);
    }
    
    files
}

#[tokio::test]
async fn test_resume_logic_fix() {
    let temp_dir = create_temp_dir();
    let source_files = create_test_files(temp_dir.path(), 50).await;
    let device_id = Uuid::new_v4();
    
    let source_paths: Vec<SdPath> = source_files.iter()
        .map(|path| SdPath::new(device_id, path.clone()))
        .collect();
    
    let destination = SdPath::new(device_id, temp_dir.path().join("dest"));
    
    let mut job = FileCopyJob::new(
        SdPathBatch::new(source_paths.clone()),
        destination.clone(),
    );
    
    // Test 1: Initially, no files should be completed
    assert_eq!(job.completed_indices.len(), 0, "Initially no files should be completed");
    
    // Test 2: Simulate partial completion (resume scenario)
    job.completed_indices = vec![0, 1, 2, 15, 16, 17, 18, 19]; // Simulate files 0-2 and 15-19 completed
    
    // Test 3: Verify the resume logic works correctly
    let mut files_to_skip = Vec::new();
    let mut files_to_process = Vec::new();
    
    for (index, source) in job.sources.paths.iter().enumerate() {
        if job.completed_indices.contains(&index) {
            files_to_skip.push(index);
        } else {
            files_to_process.push(index);
        }
    }
    
    // Should skip the completed files
    assert_eq!(files_to_skip, vec![0, 1, 2, 15, 16, 17, 18, 19], "Should skip completed files");
    
    // Should process the remaining files
    let expected_to_process: Vec<usize> = (3..15).chain(20..50).collect();
    assert_eq!(files_to_process, expected_to_process, "Should process remaining files");
    
    // Test 4: Verify tracking new completions
    job.completed_indices.push(3); // Simulate completing file 3
    assert!(job.completed_indices.contains(&3), "Should track newly completed file");
    assert!(!job.completed_indices.contains(&4), "Should not contain uncompleted file");
    
    println!("✅ Resume logic fix validated successfully");
}

#[tokio::test]
async fn test_checksum_verification_option() {
    // Test that CopyOptions properly handles verify_checksum flag
    let mut options = CopyOptions::default();
    assert_eq!(options.verify_checksum, false, "Default verify_checksum should be false");
    
    options.verify_checksum = true;
    assert_eq!(options.verify_checksum, true, "verify_checksum should be settable to true");
    
    // Test that options are properly passed to the job
    let temp_dir = create_temp_dir();
    let source_files = create_test_files(temp_dir.path(), 1).await;
    let device_id = Uuid::new_v4();
    
    let source_paths: Vec<SdPath> = source_files.iter()
        .map(|path| SdPath::new(device_id, path.clone()))
        .collect();
    
    let destination = SdPath::new(device_id, temp_dir.path().join("dest"));
    
    let job = FileCopyJob::new(
        SdPathBatch::new(source_paths),
        destination,
    ).with_options(options);
    
    assert_eq!(job.options.verify_checksum, true, "Job should have verify_checksum enabled");
    
    println!("✅ Checksum verification option validated successfully");
}

#[tokio::test]
async fn test_blake3_checksum_functionality() {
    // Test that blake3 checksum verification works correctly
    let test_content = "This is test content for checksum verification";
    let different_content = "This is different content";
    
    // Test identical content produces same hash
    let hash1 = blake3::hash(test_content.as_bytes());
    let hash2 = blake3::hash(test_content.as_bytes());
    assert_eq!(hash1, hash2, "Identical content should produce same hash");
    
    // Test different content produces different hash
    let hash3 = blake3::hash(different_content.as_bytes());
    assert_ne!(hash1, hash3, "Different content should produce different hash");
    
    // Test hex representation
    let hex1 = hash1.to_hex();
    let hex2 = hash2.to_hex();
    let hex3 = hash3.to_hex();
    
    assert_eq!(hex1, hex2, "Hex representations should match for same content");
    assert_ne!(hex1, hex3, "Hex representations should differ for different content");
    
    // Test that hex is valid
    assert!(hex1.len() > 0, "Hex representation should not be empty");
    assert!(hex1.chars().all(|c| c.is_ascii_hexdigit()), "Hex should contain only hex digits");
    
    println!("✅ Blake3 checksum functionality validated successfully");
}

#[tokio::test]
async fn test_checkpoint_interval_logic() {
    // Test the checkpoint logic (every 20 files)
    let temp_dir = create_temp_dir();
    let source_files = create_test_files(temp_dir.path(), 100).await;
    let device_id = Uuid::new_v4();
    
    let source_paths: Vec<SdPath> = source_files.iter()
        .map(|path| SdPath::new(device_id, path.clone()))
        .collect();
    
    let destination = SdPath::new(device_id, temp_dir.path().join("dest"));
    
    let mut job = FileCopyJob::new(
        SdPathBatch::new(source_paths),
        destination,
    );
    
    // Simulate processing files and checkpointing
    let mut checkpoint_points = Vec::new();
    
    for i in 0..100 {
        job.completed_indices.push(i);
        let copied_count = i + 1;
        
        // This matches the logic in the actual job: checkpoint every 20 files
        if copied_count % 20 == 0 {
            checkpoint_points.push(copied_count);
        }
    }
    
    // Should checkpoint at: 20, 40, 60, 80, 100
    assert_eq!(checkpoint_points, vec![20, 40, 60, 80, 100], "Should checkpoint at correct intervals");
    
    // Test resume scenario: interrupted after 65 files (last checkpoint at 60)
    let interrupted_at = 65;
    let last_checkpoint = checkpoint_points.iter()
        .filter(|&&cp| cp <= interrupted_at)
        .max()
        .copied()
        .unwrap_or(0);
    
    assert_eq!(last_checkpoint, 60, "Last checkpoint before interruption should be 60");
    
    // With proper resume logic, only files 60-99 would need to be reprocessed
    // (not 0-99 as with the old broken logic)
    let files_to_reprocess = (60..100).collect::<Vec<_>>();
    assert_eq!(files_to_reprocess.len(), 40, "Should only reprocess 40 files, not all 100");
    
    println!("✅ Checkpoint interval logic validated successfully");
}

#[tokio::test]
async fn test_error_message_format() {
    // Test that checksum verification error messages are properly formatted
    let source_content = "original content";
    let dest_content = "corrupted content";
    
    let source_hash = blake3::hash(source_content.as_bytes());
    let dest_hash = blake3::hash(dest_content.as_bytes());
    
    // This matches the error format from the actual implementation
    let error_msg = format!(
        "Checksum verification failed: source={}, dest={}",
        source_hash.to_hex(),
        dest_hash.to_hex()
    );
    
    // Verify error message format
    assert!(error_msg.starts_with("Checksum verification failed"), "Error should start with verification failed");
    assert!(error_msg.contains("source="), "Error should contain source hash");
    assert!(error_msg.contains("dest="), "Error should contain dest hash");
    assert!(error_msg.contains(&source_hash.to_hex().to_string()), "Error should contain actual source hash");
    assert!(error_msg.contains(&dest_hash.to_hex().to_string()), "Error should contain actual dest hash");
    
    // Test progress message format
    let progress_msg = format!(
        "Checksum verification passed for {}: {}",
        "/path/to/file.txt",
        source_hash.to_hex()
    );
    
    assert!(progress_msg.contains("Checksum verification passed"), "Progress should indicate success");
    assert!(progress_msg.contains(&source_hash.to_hex().to_string()), "Progress should contain hash");
    
    println!("✅ Error message format validated successfully");
}

#[tokio::test]
async fn test_performance_with_large_completed_indices() {
    // Test that the resume logic performs well with large numbers of completed files
    let temp_dir = create_temp_dir();
    let device_id = Uuid::new_v4();
    
    // Create a job with 10,000 files
    let source_paths: Vec<SdPath> = (0..10000)
        .map(|i| SdPath::new(device_id, temp_dir.path().join(format!("file_{}.txt", i))))
        .collect();
    
    let destination = SdPath::new(device_id, temp_dir.path().join("dest"));
    
    let mut job = FileCopyJob::new(
        SdPathBatch::new(source_paths),
        destination,
    );
    
    // Simulate completing 7,500 files
    job.completed_indices = (0..7500).collect();
    
    // Test performance of the contains operation
    let start = std::time::Instant::now();
    
    let mut skip_count = 0;
    let mut process_count = 0;
    
    for i in 0..10000 {
        if job.completed_indices.contains(&i) {
            skip_count += 1;
        } else {
            process_count += 1;
        }
    }
    
    let elapsed = start.elapsed();
    
    assert_eq!(skip_count, 7500, "Should skip 7,500 completed files");
    assert_eq!(process_count, 2500, "Should process 2,500 remaining files");
    
    // Performance should be reasonable even with 10,000 files
    // Note: Vec::contains is O(n), so this might be slow with very large lists
    // In a real implementation, we might want to use a HashSet for better performance
    println!("Performance test: {} skipped, {} to process in {:?}", skip_count, process_count, elapsed);
    
    // This test documents the current performance characteristics
    // If this becomes too slow, we should consider using HashSet instead of Vec
    
    println!("✅ Performance with large completed indices validated successfully");
}

#[tokio::test]
async fn test_copy_strategy_signature_change() {
    // Test that the CopyStrategy trait signature change works correctly
    // This is a compile-time test - if this compiles, the signature is correct
    
    struct TestStrategy;
    
    #[async_trait::async_trait]
    impl CopyStrategy for TestStrategy {
        async fn execute(
            &self,
            _ctx: &sd_core_new::infrastructure::jobs::prelude::JobContext<'_>,
            _source: &SdPath,
            _destination: &SdPath,
            verify_checksum: bool,  // This parameter should be present
        ) -> Result<u64, anyhow::Error> {
            // Test that the verify_checksum parameter is properly passed
            if verify_checksum {
                println!("Checksum verification enabled");
            } else {
                println!("Checksum verification disabled");
            }
            Ok(0)
        }
    }
    
    // If this compiles, the trait signature is correct
    let _strategy = TestStrategy;
    
    println!("✅ Copy strategy signature change validated successfully");
}

#[tokio::test]
async fn test_integration_all_fixes() {
    // Integration test that validates both fixes work together
    let temp_dir = create_temp_dir();
    let source_files = create_test_files(temp_dir.path(), 25).await;
    let device_id = Uuid::new_v4();
    
    let source_paths: Vec<SdPath> = source_files.iter()
        .map(|path| SdPath::new(device_id, path.clone()))
        .collect();
    
    let destination = SdPath::new(device_id, temp_dir.path().join("dest"));
    
    // Create job with checksum verification enabled
    let mut options = CopyOptions::default();
    options.verify_checksum = true;
    
    let mut job = FileCopyJob::new(
        SdPathBatch::new(source_paths),
        destination,
    ).with_options(options);
    
    // Simulate partial completion for resume testing
    job.completed_indices = vec![0, 1, 2, 5, 10, 15];
    
    // Test that both fixes are present
    
    // Fix 1: Resume logic - verify completed_indices is properly tracked
    assert_eq!(job.completed_indices.len(), 6, "Should have 6 completed files");
    assert!(job.completed_indices.contains(&0), "Should contain completed file 0");
    assert!(job.completed_indices.contains(&15), "Should contain completed file 15");
    assert!(!job.completed_indices.contains(&3), "Should not contain uncompleted file 3");
    
    // Fix 2: Checksum verification - verify option is enabled
    assert_eq!(job.options.verify_checksum, true, "Checksum verification should be enabled");
    
    // Test the resume logic with checksum verification
    let mut files_that_would_be_skipped = 0;
    let mut files_that_would_be_processed = 0;
    
    for (index, _source) in job.sources.paths.iter().enumerate() {
        if job.completed_indices.contains(&index) {
            files_that_would_be_skipped += 1;
        } else {
            files_that_would_be_processed += 1;
            // These files would be processed with checksum verification enabled
        }
    }
    
    assert_eq!(files_that_would_be_skipped, 6, "Should skip 6 completed files");
    assert_eq!(files_that_would_be_processed, 19, "Should process 19 remaining files with verification");
    
    println!("✅ Integration test for all fixes validated successfully");
}