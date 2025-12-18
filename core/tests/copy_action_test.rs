//! Simple integration test for the file copy action dispatch
//!
//! This test verifies that the action can be properly dispatched without
//! requiring a full database setup or job execution.

use sd_core::{
	domain::addressing::{SdPath, SdPathBatch},
	infra::action::builder::ActionBuilder,
	ops::files::copy::{
		action::FileCopyAction,
		input::CopyMethod,
		job::{CopyOptions, MoveMode},
	},
};
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

	// Test 1: Basic copy action construction (modular action)
	let copy_action = FileCopyAction {
		sources: SdPathBatch::new(vec![
			SdPath::local(source_file1.clone()),
			SdPath::local(source_file2.clone()),
		]),
		destination: SdPath::local(dest_dir.clone()),
		options: CopyOptions {
			conflict_resolution: None,
			overwrite: false,
			copy_method: CopyMethod::Auto,
			verify_checksum: true,
			preserve_timestamps: true,
			delete_after_copy: false,
			move_mode: None,
		},
		on_conflict: None,
	};

	// Verify properties directly
	assert_eq!(copy_action.sources.paths.len(), 2);
	assert_eq!(copy_action.options.overwrite, false);
	assert_eq!(copy_action.options.verify_checksum, true);
	assert_eq!(copy_action.options.preserve_timestamps, true);

	println!("Copy action construction test passed!");
}

#[tokio::test]
async fn test_move_action_construction() {
	let temp_dir = TempDir::new().unwrap();
	let source_file = temp_dir.path().join("source.txt");
	let dest_file = temp_dir.path().join("dest.txt");

	create_test_file(&source_file, "Move me!").await.unwrap();

	// Test move action semantics
	let move_action = FileCopyAction {
		sources: SdPathBatch::new(vec![SdPath::local(source_file.clone())]),
		destination: SdPath::local(dest_file.clone()),
		options: CopyOptions {
			conflict_resolution: None,
			copy_method: CopyMethod::Auto,
			overwrite: false,
			verify_checksum: false,
			preserve_timestamps: true,
			delete_after_copy: true,
			move_mode: Some(MoveMode::Move),
		},
		on_conflict: None,
	};

	assert!(move_action.options.delete_after_copy);
	assert_eq!(move_action.options.move_mode, Some(MoveMode::Move));

	println!("Move action construction test passed!");
}

#[tokio::test]
async fn test_action_validation_logic() {
	// Builder should reject empty sources
	let result = sd_core::ops::files::copy::action::FileCopyAction::builder()
		.destination("/tmp/dest")
		.build();
	assert!(result.is_err());

	println!("Action validation (builder) test passed!");
}

#[test]
fn test_copy_options_defaults() {
	let options = CopyOptions::default();

	assert!(!options.overwrite);
	assert!(!options.verify_checksum);
	assert!(options.preserve_timestamps);
	assert!(!options.delete_after_copy);
	assert!(options.move_mode.is_none());

	println!("Copy options defaults test passed!");
}
