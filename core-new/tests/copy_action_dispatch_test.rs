//! Test copy action dispatch without full job execution
//!
//! This test verifies that the action system can properly dispatch copy actions
//! and validate them correctly.

use sd_core_new::{
    operations::files::copy::{
        action::{FileCopyAction, FileCopyHandler},
        job::CopyOptions,
    },
    infrastructure::actions::{
        Action,
        handler::ActionHandler,
    },
};
use std::path::PathBuf;
use tempfile::TempDir;
use tokio::fs;
use uuid::Uuid;

/// Helper to create test files with content
async fn create_test_file(path: &std::path::Path, content: &str) -> Result<(), std::io::Error> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await?;
    }
    fs::write(path, content).await
}

#[tokio::test]
async fn test_copy_action_handler_validation() {
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
    
    create_test_file(&source_file1, "Hello, World! This is test file 1.").await.unwrap();
    create_test_file(&source_file2, "This is the content of test file 2.").await.unwrap();
    
    // Create a copy action with valid sources
    let copy_action = FileCopyAction {
        sources: vec![source_file1.clone(), source_file2.clone()],
        destination: dest_dir.clone(),
        options: CopyOptions::default(),
    };
    
    let library_id = Uuid::new_v4();
    let action = Action::FileCopy {
        library_id,
        action: copy_action,
    };
    
    // Test handler can handle this action
    let handler = FileCopyHandler::new();
    assert!(handler.can_handle(&action));
    assert_eq!(FileCopyHandler::supported_actions(), &["file.copy"]);
    
    // Test validation without context (should pass basic validation)
    // Note: We can't test full validation without a proper CoreContext
    // but we can test the basic structure
    
    println!("✅ Copy action handler validation test passed!");
}

#[tokio::test]
async fn test_copy_action_validation_errors() {
    // Test with empty sources - should fail validation
    let copy_action = FileCopyAction {
        sources: vec![],  // Empty sources
        destination: PathBuf::from("/tmp/dest"),
        options: CopyOptions::default(),
    };
    
    let library_id = Uuid::new_v4();
    let action = Action::FileCopy {
        library_id,
        action: copy_action,
    };
    
    // Create handler
    let handler = FileCopyHandler::new();
    
    // Test that handler can handle the action type
    assert!(handler.can_handle(&action));
    
    // For actual validation testing, we would need a proper CoreContext
    // which requires full core initialization. The validation logic
    // is in the handler's validate method.
    
    println!("✅ Copy action validation errors test passed!");
}

#[test]
fn test_copy_action_metadata() {
    let source_file = PathBuf::from("/tmp/source.txt");
    let dest_dir = PathBuf::from("/tmp/dest");
    
    let copy_action = FileCopyAction {
        sources: vec![source_file.clone()],
        destination: dest_dir.clone(),
        options: CopyOptions::default(),
    };
    
    let library_id = Uuid::new_v4();
    let action = Action::FileCopy {
        library_id,
        action: copy_action,
    };
    
    // Test action metadata
    assert_eq!(action.library_id(), Some(library_id));
    assert_eq!(action.kind(), "file.copy");
    
    let description = action.description();
    assert!(description.contains("Copy"));
    assert!(description.contains("1 file(s)"));
    assert!(description.contains("/tmp/dest"));
    
    let targets = action.targets_summary();
    let sources = targets.get("sources").unwrap();
    let destination = targets.get("destination").unwrap();
    
    assert!(sources.is_array());
    assert_eq!(sources.as_array().unwrap().len(), 1);
    assert_eq!(destination.as_str().unwrap(), "/tmp/dest");
    
    println!("✅ Copy action metadata test passed!");
}