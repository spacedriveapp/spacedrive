//! Integration test for delete strategies
//!
//! Tests the strategy pattern implementation for file deletion operations,
//! including local deletion and strategy routing.

use bytes::Bytes;
use sd_core::{
	domain::addressing::SdPath,
	ops::files::delete::{routing::DeleteStrategyRouter, strategy::LocalDeleteStrategy},
	volume::backend::{CloudBackend, CloudServiceType, VolumeBackend},
};
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use tokio::fs;

/// Helper to create test files with content
async fn create_test_file(path: &Path, content: &str) -> Result<(), std::io::Error> {
	if let Some(parent) = path.parent() {
		fs::create_dir_all(parent).await?;
	}
	fs::write(path, content).await
}

/// Helper to create test directory structure
async fn create_test_directory(path: &Path) -> Result<(), std::io::Error> {
	fs::create_dir_all(path).await?;

	// Create some files in the directory
	create_test_file(&path.join("file1.txt"), "Content 1").await?;
	create_test_file(&path.join("file2.txt"), "Content 2").await?;
	create_test_file(&path.join("subdir").join("file3.txt"), "Content 3").await?;

	Ok(())
}

#[tokio::test]
async fn test_local_delete_strategy_permanent() {
	// Setup test environment
	let temp_dir = TempDir::new().unwrap();
	let test_root = temp_dir.path();

	// Create test files
	let test_file1 = test_root.join("test1.txt");
	let test_file2 = test_root.join("test2.txt");

	create_test_file(&test_file1, "Test content 1")
		.await
		.unwrap();
	create_test_file(&test_file2, "Test content 2")
		.await
		.unwrap();

	// Verify files exist
	assert!(test_file1.exists());
	assert!(test_file2.exists());

	// Execute deletion using LocalDeleteStrategy
	let strategy = LocalDeleteStrategy;
	let result1 = strategy.permanent_delete(&test_file1).await;
	let result2 = strategy.permanent_delete(&test_file2).await;

	// Verify results
	assert!(result1.is_ok());
	assert!(result2.is_ok());

	// Verify files are deleted
	assert!(!test_file1.exists());
	assert!(!test_file2.exists());

	println!("test_local_delete_strategy_permanent passed!");
}

#[tokio::test]
async fn test_local_delete_strategy_trash() {
	// Setup test environment
	let temp_dir = TempDir::new().unwrap();
	let test_root = temp_dir.path();

	// Create test file
	let test_file = test_root.join("trash_test.txt");
	create_test_file(&test_file, "Trash me!").await.unwrap();

	assert!(test_file.exists());

	// Execute deletion using trash mode
	let strategy = LocalDeleteStrategy;
	let result = strategy.move_to_trash(&test_file).await;

	// Verify result - print error if it fails
	if let Err(e) = &result {
		eprintln!("Trash test failed with error: {}", e);
	}
	assert!(result.is_ok(), "move_to_trash failed: {:?}", result);

	// Verify file is moved to trash (not in original location)
	assert!(!test_file.exists());

	// Note: We can't easily verify the file is in the trash without
	// platform-specific logic, but we verified it's no longer in the original location

	println!("test_local_delete_strategy_trash passed!");
}

#[tokio::test]
async fn test_local_delete_strategy_directory() {
	// Setup test environment
	let temp_dir = TempDir::new().unwrap();
	let test_root = temp_dir.path();

	// Create test directory with files
	let test_dir = test_root.join("test_directory");
	create_test_directory(&test_dir).await.unwrap();

	// Verify directory and files exist
	assert!(test_dir.exists());
	assert!(test_dir.join("file1.txt").exists());
	assert!(test_dir.join("subdir").join("file3.txt").exists());

	// Execute deletion
	let strategy = LocalDeleteStrategy;
	let result = strategy.permanent_delete(&test_dir).await;

	// Verify result
	assert!(result.is_ok());

	// Verify directory is deleted
	assert!(!test_dir.exists());

	println!("test_local_delete_strategy_directory passed!");
}

#[tokio::test]
async fn test_delete_strategy_router_local_paths() {
	// Setup test environment
	let temp_dir = TempDir::new().unwrap();
	let test_file = temp_dir.path().join("test.txt");

	// Create local paths
	let paths = vec![
		SdPath::local(test_file.clone()),
		SdPath::local(temp_dir.path().join("test2.txt")),
	];

	// Select strategy - should return LocalDeleteStrategy for local paths
	let _strategy = DeleteStrategyRouter::select_strategy(&paths, None).await;

	// Verify description
	let description = DeleteStrategyRouter::describe_strategy(&paths).await;
	assert_eq!(description, "Local deletion");

	println!("test_delete_strategy_router_local_paths passed!");
}

#[tokio::test]
async fn test_delete_strategy_router_description() {
	// Test local paths
	let local_paths = vec![
		SdPath::local(PathBuf::from("/tmp/test1.txt")),
		SdPath::local(PathBuf::from("/tmp/test2.txt")),
	];

	let description = DeleteStrategyRouter::describe_strategy(&local_paths).await;
	assert_eq!(description, "Local deletion");

	println!("test_delete_strategy_router_description passed!");
}

#[tokio::test]
async fn test_delete_modes_all_types() {
	// Test all three delete modes work correctly
	let temp_dir = TempDir::new().unwrap();
	let test_root = temp_dir.path();

	let strategy = LocalDeleteStrategy;

	// Test 1: Permanent delete
	let perm_file = test_root.join("permanent.txt");
	create_test_file(&perm_file, "Permanent").await.unwrap();
	let result = strategy.permanent_delete(&perm_file).await;
	assert!(result.is_ok());
	assert!(!perm_file.exists());

	// Test 2: Trash delete
	let trash_file = test_root.join("trash.txt");
	create_test_file(&trash_file, "Trash").await.unwrap();
	let result = strategy.move_to_trash(&trash_file).await;
	if let Err(e) = &result {
		eprintln!("Trash test failed with error: {}", e);
	}
	assert!(result.is_ok(), "move_to_trash failed: {:?}", result);
	assert!(!trash_file.exists());

	// Test 3: Secure delete
	let secure_file = test_root.join("secure.txt");
	create_test_file(&secure_file, "Secure delete test content")
		.await
		.unwrap();
	let result = strategy.secure_delete(&secure_file).await;
	assert!(result.is_ok());
	assert!(!secure_file.exists());

	println!("test_delete_modes_all_types passed!");
}

#[tokio::test]
async fn test_strategy_error_handling() {
	// Test that strategies properly handle and report errors
	let strategy = LocalDeleteStrategy;

	// Try to delete a non-existent file
	let nonexistent = PathBuf::from("/tmp/definitely_does_not_exist_12345.txt");

	let result = strategy.permanent_delete(&nonexistent).await;

	// Should return an error
	assert!(result.is_err());

	println!("test_strategy_error_handling passed!");
}

#[tokio::test]
async fn test_cloud_backend_delete_file() {
	// Create a memory-based cloud backend for testing
	let operator = opendal::Operator::new(opendal::services::Memory::default())
		.expect("Failed to create memory operator")
		.finish();

	let backend = CloudBackend::from_operator(operator, CloudServiceType::S3);

	// Write a test file
	let test_path = Path::new("test_file.txt");
	let test_data = Bytes::from("Cloud test data");

	backend.write(test_path, test_data.clone()).await.unwrap();

	// Verify file exists
	assert!(backend.exists(test_path).await.unwrap());

	// Delete the file
	let result = backend.delete(test_path).await;
	assert!(result.is_ok(), "Delete failed: {:?}", result);

	// Verify file no longer exists
	assert!(!backend.exists(test_path).await.unwrap());

	println!("test_cloud_backend_delete_file passed!");
}

#[tokio::test]
async fn test_cloud_backend_delete_directory() {
	// Create a memory-based cloud backend for testing
	let operator = opendal::Operator::new(opendal::services::Memory::default())
		.expect("Failed to create memory operator")
		.finish();

	let backend = CloudBackend::from_operator(operator, CloudServiceType::S3);

	// Write files in a directory structure
	backend
		.write(Path::new("test_dir/file1.txt"), Bytes::from("File 1"))
		.await
		.unwrap();
	backend
		.write(Path::new("test_dir/file2.txt"), Bytes::from("File 2"))
		.await
		.unwrap();
	backend
		.write(
			Path::new("test_dir/subdir/file3.txt"),
			Bytes::from("File 3"),
		)
		.await
		.unwrap();

	// Verify files exist
	assert!(backend
		.exists(Path::new("test_dir/file1.txt"))
		.await
		.unwrap());
	assert!(backend
		.exists(Path::new("test_dir/file2.txt"))
		.await
		.unwrap());

	// Delete the entire directory
	let result = backend.delete(Path::new("test_dir/")).await;
	assert!(result.is_ok(), "Directory delete failed: {:?}", result);

	// Verify directory and files no longer exist
	assert!(!backend
		.exists(Path::new("test_dir/file1.txt"))
		.await
		.unwrap());
	assert!(!backend
		.exists(Path::new("test_dir/file2.txt"))
		.await
		.unwrap());

	println!("test_cloud_backend_delete_directory passed!");
}
