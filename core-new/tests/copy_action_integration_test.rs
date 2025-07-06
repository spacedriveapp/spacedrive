//! Integration tests for the file copy action system
//!
//! This test verifies the complete flow from action dispatch through job execution,
//! file system operations, and audit logging.

use sd_core_new::{
    Core,
    infrastructure::{
        actions::{
            Action, 
            manager::ActionManager,
        },
        database::entities::{audit_log, AuditLog},
        jobs::types::{JobId, JobStatus},
    },
    operations::files::copy::{
        action::FileCopyAction,
        job::{CopyOptions, MoveMode},
    },
};
use sea_orm::{EntityTrait, QuerySelect};
use std::{
    path::PathBuf,
    sync::Arc,
    time::Duration,
};
use tempfile::TempDir;
use tokio::{fs, time::timeout};
use uuid::Uuid;

/// Helper to create test files with content
async fn create_test_file(path: &std::path::Path, content: &str) -> Result<(), std::io::Error> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await?;
    }
    fs::write(path, content).await
}

/// Helper to verify file content matches expected
async fn verify_file_content(path: &std::path::Path, expected: &str) -> Result<bool, std::io::Error> {
    let content = fs::read_to_string(path).await?;
    Ok(content == expected)
}

#[tokio::test]
async fn test_copy_action_full_integration() {
    // Setup test environment
    let temp_dir = TempDir::new().unwrap();
    let test_root = temp_dir.path();
    
    // Create source and destination directories
    let source_dir = test_root.join("source");
    let dest_dir = test_root.join("destination");
    fs::create_dir_all(&source_dir).await.unwrap();
    fs::create_dir_all(&dest_dir).await.unwrap();
    
    // Create test files
    let source_file1 = source_dir.join("test1.txt");
    let source_file2 = source_dir.join("test2.txt");
    let dest_file1 = dest_dir.join("test1.txt");
    let dest_file2 = dest_dir.join("test2.txt");
    
    create_test_file(&source_file1, "Hello, World! This is test file 1.").await.unwrap();
    create_test_file(&source_file2, "This is the content of test file 2.").await.unwrap();
    
    // Initialize core with custom data directory
    let core_data_dir = test_root.join("core_data");
    let core = Core::new_with_config(core_data_dir)
        .await
        .unwrap();
    
    // Create a test library
    let library = core
        .libraries
        .create_library("Copy Test Library", None)
        .await
        .unwrap();
    
    let library_id = library.id();
    
    // Create ActionManager
    let context = core.context.clone();
    let action_manager = ActionManager::new(context);
    
    // Build the copy action
    let copy_action = FileCopyAction {
        sources: vec![source_file1.clone(), source_file2.clone()],
        destination: dest_dir.clone(),
        options: CopyOptions {
            overwrite: false,
            verify_checksum: true,
            preserve_timestamps: true,
            delete_after_copy: false,
            move_mode: None,
        },
    };
    
    // Create the Action enum with library context
    let action = Action::FileCopy {
        library_id,
        action: copy_action,
    };
    
    // Record initial state
    let initial_audit_count = count_audit_entries(&library, library_id).await;
    
    // Verify source files exist and destination files don't
    assert!(source_file1.exists());
    assert!(source_file2.exists());
    assert!(!dest_file1.exists());
    assert!(!dest_file2.exists());
    
    // ===== Execute the action =====
    let action_output = action_manager
        .dispatch(action)
        .await
        .expect("Action dispatch should succeed");
    
    // Verify action output
    assert_eq!(action_output.output_type, "file.copy.dispatched");
    assert!(action_output.data.get("job_id").is_some());
    assert!(action_output.message.contains("Dispatched file copy job"));
    
    // Extract job ID from output
    let job_id_value = action_output.data.get("job_id").unwrap();
    let job_id_str = job_id_value.as_str().expect("job_id should be a string");
    let job_id = Uuid::parse_str(job_id_str).expect("job_id should be valid UUID");
    
    // ===== Wait for job completion =====
    // Poll job status until completion (with timeout)
    let job_completion = timeout(Duration::from_secs(30), async {
        loop {
            if let Some(job_handle) = library.jobs().get_job(JobId::from(job_id)).await {
                let status = job_handle.status();
                if matches!(status, JobStatus::Completed | JobStatus::Failed) {
                    return status;
                }
            }
            
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("Job should complete within timeout");
    
    // Verify job completed successfully
    assert!(matches!(job_completion, JobStatus::Completed), "Job should complete successfully");
    
    // ===== Verify file system changes =====
    // Check that files were copied successfully
    assert!(dest_file1.exists(), "Destination file 1 should exist");
    assert!(dest_file2.exists(), "Destination file 2 should exist");
    
    // Verify file contents match
    assert!(
        verify_file_content(&dest_file1, "Hello, World! This is test file 1.")
            .await
            .unwrap(),
        "Destination file 1 content should match source"
    );
    assert!(
        verify_file_content(&dest_file2, "This is the content of test file 2.")
            .await
            .unwrap(),
        "Destination file 2 content should match source"
    );
    
    // Verify source files still exist (copy, not move)
    assert!(source_file1.exists(), "Source file 1 should still exist");
    assert!(source_file2.exists(), "Source file 2 should still exist");
    
    // ===== Verify audit log =====
    let final_audit_count = count_audit_entries(&library, library_id).await;
    assert_eq!(
        final_audit_count, 
        initial_audit_count + 1,
        "Should have one new audit log entry"
    );
    
    // Get the audit log entry
    let audit_entries = get_recent_audit_entries(&library, library_id, 1).await;
    assert_eq!(audit_entries.len(), 1, "Should have exactly one audit entry");
    
    let audit_entry = &audit_entries[0];
    assert_eq!(audit_entry.action_type, "file.copy");
    assert_eq!(audit_entry.status, audit_log::ActionStatus::Completed);
    assert!(audit_entry.job_id.is_some(), "Audit entry should have job_id");
    assert_eq!(audit_entry.job_id.as_ref().unwrap(), &job_id.to_string());
    assert!(audit_entry.completed_at.is_some(), "Audit entry should have completion time");
    assert!(audit_entry.error_message.is_none(), "Audit entry should not have error message");
    
    // Verify audit entry targets contain source and destination info (now stored as JSON string)
    let targets_json: serde_json::Value = serde_json::from_str(&audit_entry.targets).unwrap();
    assert!(targets_json.get("sources").is_some(), "Audit should contain sources");
    assert!(targets_json.get("destination").is_some(), "Audit should contain destination");
    
    let sources = targets_json.get("sources").unwrap().as_array().unwrap();
    assert_eq!(sources.len(), 2, "Should have 2 source files in audit");
    
    println!("✅ Copy action integration test passed!");
    println!("   - Action dispatched successfully");
    println!("   - Job executed and completed");
    println!("   - Files copied correctly");
    println!("   - Audit log entry created");
}

#[tokio::test]
async fn test_copy_action_with_move_operation() {
    // Setup test environment
    let temp_dir = TempDir::new().unwrap();
    let test_root = temp_dir.path();
    
    let source_dir = test_root.join("source");
    let dest_dir = test_root.join("destination");
    fs::create_dir_all(&source_dir).await.unwrap();
    fs::create_dir_all(&dest_dir).await.unwrap();
    
    // Create test file
    let source_file = source_dir.join("move_test.txt");
    let dest_file = dest_dir.join("move_test.txt");
    
    create_test_file(&source_file, "This file will be moved.").await.unwrap();
    
    // Initialize core and library
    let core_data_dir = test_root.join("core_data");
    let core = Core::new_with_config(core_data_dir).await.unwrap();
    let library = core
        .libraries
        .create_library("Move Test Library", None)
        .await
        .unwrap();
    let library_id = library.id();
    
    // Create ActionManager
    let context = core.context.clone();
    let action_manager = ActionManager::new(context);
    
    // Build move action (copy with delete_after_copy)
    let copy_action = FileCopyAction {
        sources: vec![source_file.clone()],
        destination: dest_file.clone(),
        options: CopyOptions {
            overwrite: false,
            verify_checksum: false,
            preserve_timestamps: true,
            delete_after_copy: true,  // This makes it a move operation
            move_mode: Some(MoveMode::Move),
        },
    };
    
    let action = Action::FileCopy {
        library_id,
        action: copy_action,
    };
    
    // Verify initial state
    assert!(source_file.exists());
    assert!(!dest_file.exists());
    
    // Execute the move action
    let action_output = action_manager
        .dispatch(action)
        .await
        .expect("Move action should succeed");
    
    // Extract and wait for job completion
    let job_id_str = action_output.data.get("job_id").unwrap().as_str().unwrap();
    let job_id = Uuid::parse_str(job_id_str).unwrap();
    
    // Wait for job completion
    timeout(Duration::from_secs(15), async {
        loop {
            if let Some(job_handle) = library.jobs().get_job(JobId::from(job_id)).await {
                let status = job_handle.status();
                if matches!(status, JobStatus::Completed | JobStatus::Failed) {
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("Move job should complete");
    
    // Verify file was moved (destination exists, source doesn't)
    assert!(dest_file.exists(), "Destination file should exist after move");
    assert!(!source_file.exists(), "Source file should not exist after move");
    
    // Verify content
    assert!(
        verify_file_content(&dest_file, "This file will be moved.")
            .await
            .unwrap(),
        "Moved file content should match"
    );
    
    println!("✅ Move operation test passed!");
}

#[tokio::test]
async fn test_copy_action_validation_errors() {
    let temp_dir = TempDir::new().unwrap();
    let core_data_dir = temp_dir.path().join("core_data");
    let core = Core::new_with_config(core_data_dir).await.unwrap();
    let library = core
        .libraries
        .create_library("Validation Test Library", None)
        .await
        .unwrap();
    let library_id = library.id();
    
    let context = core.context.clone();
    let action_manager = ActionManager::new(context);
    
    // Test 1: Empty sources should fail validation
    let invalid_action = Action::FileCopy {
        library_id,
        action: FileCopyAction {
            sources: vec![],  // Empty sources
            destination: PathBuf::from("/tmp/dest"),
            options: CopyOptions::default(),
        },
    };
    
    let result = action_manager.dispatch(invalid_action).await;
    assert!(result.is_err(), "Empty sources should cause validation error");
    
    let error = result.unwrap_err();
    assert!(error.to_string().contains("At least one source"), "Error should mention source requirement");
    
    println!("✅ Validation error test passed!");
}

/// Helper function to count audit log entries for a library
async fn count_audit_entries(
    library: &Arc<sd_core_new::library::Library>,
    _library_id: Uuid,
) -> usize {
    let db = library.db().conn();
    
    AuditLog::find()
        .all(db)
        .await
        .unwrap_or_default()
        .len()
}

/// Helper function to get recent audit log entries
async fn get_recent_audit_entries(
    library: &Arc<sd_core_new::library::Library>,
    _library_id: Uuid,
    limit: u64,
) -> Vec<audit_log::Model> {
    let db = library.db().conn();
    
    AuditLog::find()
        .limit(limit)
        .all(db)
        .await
        .unwrap_or_default()
}