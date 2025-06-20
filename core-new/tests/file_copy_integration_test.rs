//! Simplified file copy integration test

use sd_core_new::{
	operations::file_ops::copy_job::FileCopyJob,
	shared::types::{set_current_device_id, SdPath, SdPathBatch},
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

/// Helper to read file content
async fn read_file_content(path: &std::path::Path) -> Result<String, std::io::Error> {
	fs::read_to_string(path).await
}

#[tokio::test]
async fn test_file_copy_basic_functionality() {
	// Initialize test environment
	let temp_dir = TempDir::new().unwrap();
	let device_id = Uuid::new_v4();
	set_current_device_id(device_id);

	// Create a test file
	let source_path = temp_dir.path().join("test_source.txt");
	let test_content = "Hello, World! This is a test file for copy operations.";
	create_test_file(&source_path, test_content).await.unwrap();

	// Create destination directory
	let dest_dir = temp_dir.path().join("destination");
	fs::create_dir_all(&dest_dir).await.unwrap();

	// Set up copy job
	let source_sd_path = SdPath::new(device_id, source_path.clone());
	let dest_sd_path = SdPath::new(device_id, dest_dir.clone());
	let sources = SdPathBatch::new(vec![source_sd_path]);

	let copy_job = FileCopyJob::new(sources, dest_sd_path);

	// Test that the job was created correctly
	assert_eq!(copy_job.sources.paths.len(), 1);
	assert_eq!(copy_job.destination.device_id, device_id);

	println!("✅ File copy job creation test passed");
}

#[tokio::test]
async fn test_file_copy_directory_structure() {
	let temp_dir = TempDir::new().unwrap();
	let device_id = Uuid::new_v4();
	set_current_device_id(device_id);

	// Create a directory with multiple files
	let source_dir = temp_dir.path().join("source_directory");

	// Create test files in the directory
	let test_files = [
		("file1.txt", "Content of file 1"),
		("file2.md", "# Markdown content"),
		("subdir/nested.txt", "Nested file content"),
	];

	for (filename, content) in &test_files {
		let file_path = source_dir.join(filename);
		create_test_file(&file_path, content).await.unwrap();
	}

	// Verify files were created
	for (filename, expected_content) in &test_files {
		let file_path = source_dir.join(filename);
		assert!(file_path.exists(), "Test file {} was not created", filename);

		let actual_content = read_file_content(&file_path).await.unwrap();
		assert_eq!(
			*expected_content, actual_content,
			"Content mismatch for {}",
			filename
		);
	}

	// Create copy job for the entire directory
	let source_sd_path = SdPath::new(device_id, source_dir);
	let dest_dir = temp_dir.path().join("copied_directory");
	let dest_sd_path = SdPath::new(device_id, dest_dir);

	let sources = SdPathBatch::new(vec![source_sd_path]);
	let copy_job = FileCopyJob::new(sources, dest_sd_path);

	// Verify job setup
	assert_eq!(copy_job.sources.paths.len(), 1);
	assert!(!copy_job.options.overwrite);
	assert!(copy_job.options.preserve_timestamps);

	println!("✅ Directory structure copy job test passed");
}

#[tokio::test]
async fn test_multiple_files_copy_setup() {
	let temp_dir = TempDir::new().unwrap();
	let device_id = Uuid::new_v4();
	set_current_device_id(device_id);

	// Create multiple source files
	let source_files = [
		("document.txt", "Document content"),
		("image.jpg", "Mock image data"),
		("config.json", r#"{"setting": "value"}"#),
	];

	let mut source_paths = Vec::new();

	for (filename, content) in &source_files {
		let file_path = temp_dir.path().join("sources").join(filename);
		create_test_file(&file_path, content).await.unwrap();
		source_paths.push(SdPath::new(device_id, file_path));
	}

	// Create destination
	let dest_dir = temp_dir.path().join("multi_destination");
	let dest_sd_path = SdPath::new(device_id, dest_dir);

	// Create copy job with multiple sources
	let sources = SdPathBatch::new(source_paths);
	let copy_job = FileCopyJob::new(sources, dest_sd_path);

	// Verify job setup
	assert_eq!(copy_job.sources.paths.len(), 3);

	// Test SdPathBatch functionality
	let by_device = copy_job.sources.by_device();
	assert_eq!(by_device.len(), 1);
	assert!(by_device.contains_key(&device_id));
	assert_eq!(by_device[&device_id].len(), 3);

	println!("✅ Multiple files copy job setup test passed");
}

#[tokio::test]
async fn test_copy_job_with_options() {
	let temp_dir = TempDir::new().unwrap();
	let device_id = Uuid::new_v4();
	set_current_device_id(device_id);

	// Create test file
	let source_path = temp_dir.path().join("test.txt");
	create_test_file(&source_path, "test content")
		.await
		.unwrap();

	let source_sd_path = SdPath::new(device_id, source_path);
	let dest_sd_path = SdPath::new(device_id, temp_dir.path().join("dest"));
	let sources = SdPathBatch::new(vec![source_sd_path]);

	// Test with different options
	let mut copy_job = FileCopyJob::new(sources, dest_sd_path);

	// Test default options
	assert!(!copy_job.options.overwrite);
	assert!(copy_job.options.preserve_timestamps);
	assert!(!copy_job.options.verify_checksum);

	// Modify options
	copy_job.options.overwrite = true;
	copy_job.options.verify_checksum = true;
	copy_job.options.preserve_timestamps = false;

	// Verify options were set
	assert!(copy_job.options.overwrite);
	assert!(copy_job.options.verify_checksum);
	assert!(!copy_job.options.preserve_timestamps);

	println!("✅ Copy job options test passed");
}

#[tokio::test]
async fn test_cross_device_detection() {
	let temp_dir = TempDir::new().unwrap();
	let device1 = Uuid::new_v4();
	let device2 = Uuid::new_v4();
	set_current_device_id(device1);

	// Create source on device 1
	let source_path = temp_dir.path().join("source.txt");
	create_test_file(&source_path, "content").await.unwrap();
	let source_sd_path = SdPath::new(device1, source_path);

	// Destination on device 2 (cross-device)
	let dest_path = temp_dir.path().join("dest");
	let dest_sd_path = SdPath::new(device2, dest_path);

	let sources = SdPathBatch::new(vec![source_sd_path]);
	let copy_job = FileCopyJob::new(sources, dest_sd_path);

	// Verify we can detect cross-device scenario
	assert_ne!(
		copy_job.sources.paths[0].device_id,
		copy_job.destination.device_id
	);

	// Test device grouping
	let by_device = copy_job.sources.by_device();
	assert_eq!(by_device.len(), 1);
	assert!(by_device.contains_key(&device1));

	println!("✅ Cross-device detection test passed");
}

#[tokio::test]
async fn test_sd_path_batch_functionality() {
	let device1 = Uuid::new_v4();
	let device2 = Uuid::new_v4();
	set_current_device_id(device1);

	// Create paths on different devices
	let paths = vec![
		SdPath::new(device1, "/path1/file1.txt"),
		SdPath::new(device1, "/path1/file2.txt"),
		SdPath::new(device2, "/path2/file3.txt"),
		SdPath::new(device2, "/path2/file4.txt"),
	];

	let batch = SdPathBatch::new(paths);

	// Test device grouping
	let by_device = batch.by_device();
	assert_eq!(by_device.len(), 2);
	assert_eq!(by_device[&device1].len(), 2);
	assert_eq!(by_device[&device2].len(), 2);

	// Test local filtering
	let local_paths = batch.local_only();
	assert_eq!(local_paths.len(), 2); // Only device1 paths should be local

	println!("✅ SdPathBatch functionality test passed");
}

#[tokio::test]
async fn test_file_operations_integration() {
	let temp_dir = TempDir::new().unwrap();
	let device_id = Uuid::new_v4();
	set_current_device_id(device_id);

	// Test the full pipeline of file copy setup

	// 1. Create source files
	let source_dir = temp_dir.path().join("sources");
	let files = [
		("readme.txt", "This is a readme file"),
		("data.csv", "name,value\ntest,123\nfoo,456"),
		("nested/config.toml", "[section]\nkey = \"value\""),
	];

	let mut sources = Vec::new();
	for (filename, content) in &files {
		let file_path = source_dir.join(filename);
		create_test_file(&file_path, content).await.unwrap();
		sources.push(SdPath::new(device_id, file_path));
	}

	// 2. Set up destination
	let dest_dir = temp_dir.path().join("destination");
	fs::create_dir_all(&dest_dir).await.unwrap();
	let dest_sd_path = SdPath::new(device_id, dest_dir.clone());

	// 3. Create and configure copy job
	let source_batch = SdPathBatch::new(sources);
	let mut copy_job = FileCopyJob::new(source_batch, dest_sd_path);
	copy_job.options.preserve_timestamps = true;
	copy_job.options.overwrite = false;

	// 4. Verify job configuration
	assert_eq!(copy_job.sources.paths.len(), 3);
	assert_eq!(copy_job.destination.device_id, device_id);
	assert!(copy_job.options.preserve_timestamps);
	assert!(!copy_job.options.overwrite);

	// 5. Test that all source files exist and have correct content
	for (i, (_, expected_content)) in files.iter().enumerate() {
		if let Some(local_path) = copy_job.sources.paths[i].as_local_path() {
			assert!(local_path.exists(), "Source file {} does not exist", i);
			let actual_content = read_file_content(local_path).await.unwrap();
			assert_eq!(
				*expected_content, actual_content,
				"Content mismatch for file {}",
				i
			);
		}
	}

	println!("✅ Complete file operations integration test passed");
}
