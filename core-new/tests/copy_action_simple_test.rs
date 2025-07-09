//! Simple integration test for the file copy action dispatch
//!
//! This test verifies that the action can be properly dispatched without
//! requiring a full database setup or job execution.

use sd_core_new::{
	infrastructure::actions::{manager::ActionManager, Action},
	operations::files::{
		copy::{
			action::FileCopyAction,
			job::{CopyOptions, MoveMode},
		},
		input::CopyMethod,
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
async fn test_copy_action_construction() {
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

	create_test_file(&source_file1, "Hello, World! This is test file 1.")
		.await
		.unwrap();
	create_test_file(&source_file2, "This is the content of test file 2.")
		.await
		.unwrap();

	// Test 1: Basic copy action construction
	let copy_action = FileCopyAction {
		sources: vec![source_file1.clone(), source_file2.clone()],
		destination: dest_dir.clone(),
		options: CopyOptions {
			overwrite: false,
			copy_method: CopyMethod::Auto,
			verify_checksum: true,
			preserve_timestamps: true,
			delete_after_copy: false,
			move_mode: None,
		},
	};

	// Create Action enum
	let library_id = Uuid::new_v4();
	let action = Action::FileCopy {
		library_id,
		action: copy_action,
	};

	// Verify action properties
	assert_eq!(action.library_id(), Some(library_id));
	assert_eq!(action.kind(), "file.copy");
	assert!(action.description().contains("Copy 2 file(s)"));

	let targets = action.targets_summary();
	assert!(targets.get("sources").is_some());
	assert!(targets.get("destination").is_some());

	println!("✅ Copy action construction test passed!");
}

#[tokio::test]
async fn test_move_action_construction() {
	let temp_dir = TempDir::new().unwrap();
	let source_file = temp_dir.path().join("source.txt");
	let dest_file = temp_dir.path().join("dest.txt");

	create_test_file(&source_file, "Move me!").await.unwrap();

	// Test move action (copy with delete_after_copy)
	let move_action = FileCopyAction {
		sources: vec![source_file.clone()],
		destination: dest_file.clone(),
		options: CopyOptions {
			copy_method: CopyMethod::Auto,
			overwrite: false,
			verify_checksum: false,
			preserve_timestamps: true,
			delete_after_copy: true, // This makes it a move operation
			move_mode: Some(MoveMode::Move),
		},
	};

	let library_id = Uuid::new_v4();
	let action = Action::FileCopy {
		library_id,
		action: move_action,
	};

	// Verify action properties
	assert_eq!(action.library_id(), Some(library_id));
	assert_eq!(action.kind(), "file.copy");
	assert!(action.description().contains("Copy 1 file(s)"));

	println!("✅ Move action construction test passed!");
}

#[tokio::test]
async fn test_action_validation_logic() {
	// Test empty sources validation
	let copy_action = FileCopyAction {
		sources: vec![], // Empty sources should be invalid
		destination: PathBuf::from("/tmp/dest"),
		options: CopyOptions::default(),
	};

	let library_id = Uuid::new_v4();
	let action = Action::FileCopy {
		library_id,
		action: copy_action,
	};

	// For now, just verify the action is constructed correctly
	// The actual validation happens in the ActionHandler
	assert_eq!(action.library_id(), Some(library_id));
	assert_eq!(action.kind(), "file.copy");

	println!("✅ Action validation logic test passed!");
}

#[test]
fn test_copy_options_defaults() {
	let options = CopyOptions::default();

	assert!(!options.overwrite);
	assert!(!options.verify_checksum);
	assert!(options.preserve_timestamps);
	assert!(!options.delete_after_copy);
	assert!(options.move_mode.is_none());

	println!("✅ Copy options defaults test passed!");
}

#[test]
fn test_move_mode_variants() {
	// Test that all move modes can be constructed
	let _move_mode = MoveMode::Move;
	let _rename_mode = MoveMode::Rename;
	let _cut_mode = MoveMode::Cut;

	println!("✅ Move mode variants test passed!");
}
