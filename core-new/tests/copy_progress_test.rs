//! Test for monitoring copy progress with large files
//!
//! This test verifies that copy progress updates smoothly with byte-level
//! granularity rather than jumping in large increments.

use sd_core_new::{
	infrastructure::{
		actions::{manager::ActionManager, Action},
		jobs::types::{JobId, JobStatus},
	},
	operations::files::{
		copy::{action::FileCopyAction, job::CopyOptions},
		input::CopyMethod,
	},
	Core,
};
use std::{
	path::PathBuf,
	sync::{Arc, Mutex},
	time::Duration,
};
use tempfile::TempDir;
use tokio::{fs, time::timeout};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;

/// Create a large test file with specified size
async fn create_large_test_file(
	path: &std::path::Path,
	size_mb: usize,
) -> Result<(), std::io::Error> {
	if let Some(parent) = path.parent() {
		fs::create_dir_all(parent).await?;
	}

	// Create file with 1MB chunks to avoid memory issues
	let chunk_size = 1024 * 1024; // 1MB
	let chunk = vec![0u8; chunk_size];

	let mut file = fs::OpenOptions::new()
		.create(true)
		.write(true)
		.truncate(true)
		.open(path)
		.await?;

	use tokio::io::AsyncWriteExt;
	for _ in 0..size_mb {
		file.write_all(&chunk).await?;
	}

	file.sync_all().await?;
	Ok(())
}

#[derive(Debug, Clone)]
struct ProgressSnapshot {
	timestamp: std::time::Instant,
	percentage: f32,
	bytes_copied: u64,
	message: String,
}

#[tokio::test]
async fn test_copy_progress_monitoring_large_file() {
	// Initialize tracing subscriber for debug logs
	let _guard = tracing_subscriber::registry()
		.with(tracing_subscriber::fmt::layer())
		.with(
			tracing_subscriber::EnvFilter::try_from_default_env()
				.unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
		)
		.set_default();

	// Setup test environment
	let temp_dir = TempDir::new().unwrap();
	let test_root = temp_dir.path();

	// Create source and destination directories
	let source_dir = test_root.join("source");
	let dest_dir = test_root.join("destination");
	fs::create_dir_all(&source_dir).await.unwrap();
	fs::create_dir_all(&dest_dir).await.unwrap();

	// Create a large test file (100MB)
	let source_file = source_dir.join("large_test_file.bin");
	let file_size_mb = 100; // 100MB

	println!("Creating {}MB test file...", file_size_mb);
	create_large_test_file(&source_file, file_size_mb)
		.await
		.unwrap();

	// Verify file size
	let metadata = fs::metadata(&source_file).await.unwrap();
	let expected_size = (file_size_mb * 1024 * 1024) as u64;
	assert_eq!(
		metadata.len(),
		expected_size,
		"Test file should be exactly {}MB",
		file_size_mb
	);

	// Initialize core with custom data directory
	let core_data_dir = test_root.join("core_data");
	let core = Core::new_with_config(core_data_dir).await.unwrap();

	// Create a test library
	let library = core
		.libraries
		.create_library("Progress Test Library", None, core.context.clone())
		.await
		.unwrap();

	let library_id = library.id();

	// Create ActionManager
	let context = core.context.clone();
	let action_manager = ActionManager::new(context);

	// Build the copy action with the exact options from the CLI command
	let copy_action = FileCopyAction {
		sources: vec![source_file.clone()],
		destination: dest_dir.clone(),
		options: CopyOptions {
			overwrite: false,
			verify_checksum: true,     // --verify
			preserve_timestamps: true, // --preserve-timestamps
			delete_after_copy: false,
			move_mode: None,
			copy_method: CopyMethod::StreamingCopy, // --method streaming
		},
	};

	// Create the Action enum with library context
	let action = Action::FileCopy {
		library_id,
		action: copy_action,
	};

	// Setup progress monitoring
	let progress_snapshots = Arc::new(Mutex::new(Vec::new()));
	let progress_snapshots_clone = progress_snapshots.clone();
	let start_time = std::time::Instant::now();

	// Execute the action
	println!("Starting copy operation...");
	let action_output = action_manager
		.dispatch(action)
		.await
		.expect("Action dispatch should succeed");

	// Extract job ID from output
	let job_id = match &action_output {
		sd_core_new::infrastructure::actions::output::ActionOutput::Custom { data, .. } => {
			let job_id_value = data.get("job_id").unwrap();
			let job_id_str = job_id_value.as_str().expect("job_id should be a string");
			Uuid::parse_str(job_id_str).expect("job_id should be valid UUID")
		}
		_ => panic!("Expected Custom ActionOutput variant"),
	};
	println!("Monitoring job ID: {}", job_id);

	// Start monitoring task
	let library_clone = library.clone();
	let expected_size_clone = expected_size;
	let monitor_handle = tokio::spawn(async move {
		let mut last_progress = 0.0;
		let mut consecutive_same_progress = 0;
		let mut poll_count = 0;
		let mut has_seen_progress = false;

		loop {
			poll_count += 1;

			// Get job info from the job manager
			let job_info_result = library_clone.jobs().get_job_info(job_id).await.unwrap();
			if let Some(job_info) = job_info_result {
				let current_progress = job_info.progress * 100.0;

				// Only show debug output every 100 polls to reduce noise
				if poll_count % 100 == 0 && poll_count > 0 {
					println!(
						"Poll #{}: Status={:?}, Progress={:.1}%",
						poll_count, job_info.status, current_progress
					);
				}

				// Debug log when we're near completion
				if current_progress > 99.0 {
					println!(
						"Near completion - Poll #{}: Status={:?}, Progress={:.1}%",
						poll_count, job_info.status, current_progress
					);
				}

				// Record snapshot if progress changed
				if (current_progress - last_progress).abs() > 0.01 {
					consecutive_same_progress = 0;
					has_seen_progress = true;

					let snapshot = ProgressSnapshot {
						timestamp: std::time::Instant::now(),
						percentage: current_progress,
						bytes_copied: (expected_size_clone as f64
							* (current_progress as f64 / 100.0)) as u64,
						message: format!("{:.1}%", current_progress),
					};

					println!(
						"Progress: {:.1}% ({} MB)",
						current_progress,
						snapshot.bytes_copied / (1024 * 1024)
					);

					progress_snapshots_clone.lock().unwrap().push(snapshot);
					last_progress = current_progress;
				}

				// Check if job is complete
				match job_info.status {
					JobStatus::Completed => {
						println!("Job completed! (after {} polls)", poll_count);
						println!("Final progress: {:.1}%", current_progress);
						// Record final progress if we haven't seen any updates
						if !has_seen_progress && current_progress > 0.0 {
							let snapshot = ProgressSnapshot {
								timestamp: std::time::Instant::now(),
								percentage: current_progress,
								bytes_copied: expected_size_clone,
								message: "Final".to_string(),
							};
							progress_snapshots_clone.lock().unwrap().push(snapshot);
						}
						break;
					}
					JobStatus::Failed => {
						println!("Job failed after {} polls", poll_count);
						panic!("Job failed!");
					}
					_ => {
						// Continue monitoring
						consecutive_same_progress += 1;

						// If progress hasn't changed for many iterations, it might be stuck
						if consecutive_same_progress == 100 {
							println!(
								"Warning: Progress appears stuck at {:.1}% after 100 polls",
								current_progress
							);
						}

						// Fail fast if progress is stuck at 0% for too long
						if consecutive_same_progress > 200 && current_progress == 0.0 {
							println!(
								"ERROR: Progress stuck at 0% for {} polls. Aborting test.",
								consecutive_same_progress
							);
							break;
						}
					}
				}
			} else {
				println!(
					"Job info returned None for job {} (poll #{})",
					job_id, poll_count
				);
				// Job might have been removed from running jobs after completion
				// Let's assume it completed successfully
				break;
			}

			// Poll every 50ms to catch fine-grained progress updates
			tokio::time::sleep(Duration::from_millis(50)).await;
		}

		has_seen_progress
	});

	// Wait for job completion with timeout
	let completion_result = timeout(Duration::from_secs(30), monitor_handle).await;

	let has_seen_progress = match completion_result {
		Ok(Ok(has_progress)) => {
			println!("Monitoring completed successfully");
			has_progress
		}
		Ok(Err(e)) => panic!("Monitoring task failed: {}", e),
		Err(_) => panic!("Copy operation timed out after 30 seconds"),
	};

	// Analyze progress snapshots
	let snapshots = progress_snapshots.lock().unwrap();
	println!("\n=== Progress Analysis ===");
	println!("Total snapshots captured: {}", snapshots.len());
	println!("Saw progress updates during copy: {}", has_seen_progress);

	// First check if we got ANY progress updates at all
	if snapshots.is_empty() {
		panic!(
			"No progress updates were captured! Progress stayed at 0% throughout the entire copy operation. \
			This indicates the progress reporting is not working correctly."
		);
	}

	// If we only got one snapshot at the end, that's also a problem
	if snapshots.len() == 1 && !has_seen_progress {
		panic!(
			"Only captured final progress update. Progress reporting did not work during the copy operation."
		);
	}

	if snapshots.len() < 10 {
		panic!(
			"Too few progress updates captured! Only {} snapshots for a {}MB file. \
			Expected smooth byte-level progress updates throughout the operation.",
			snapshots.len(),
			file_size_mb
		);
	}

	// Calculate progress increments
	let mut increments = Vec::new();
	for i in 1..snapshots.len() {
		let increment = snapshots[i].percentage - snapshots[i - 1].percentage;
		if increment > 0.0 {
			increments.push(increment);
		}
	}

	// Calculate statistics
	let avg_increment = increments.iter().sum::<f32>() / increments.len() as f32;
	let max_increment = increments.iter().cloned().fold(0.0f32, f32::max);
	let min_increment = increments.iter().cloned().fold(100.0f32, f32::min);

	println!("Progress increments:");
	println!("  Average: {:.2}%", avg_increment);
	println!("  Maximum: {:.2}%", max_increment);
	println!("  Minimum: {:.2}%", min_increment);
	println!("  Total updates: {}", increments.len());

	// Verify smooth progress (no large jumps)
	// For a 1GB file, we should see many small increments
	// A 25% jump would indicate file-based progress instead of byte-based
	assert!(
		max_increment < 10.0,
		"Progress jumped by {:.1}% - should update smoothly with byte-level granularity",
		max_increment
	);

	// Verify we got reasonable granularity
	assert!(
		snapshots.len() > 20,
		"Expected at least 20 progress updates for a {}MB file, got {}",
		file_size_mb,
		snapshots.len()
	);

	// Verify file was copied successfully
	let dest_file = dest_dir.join("large_test_file.bin");
	assert!(dest_file.exists(), "Destination file should exist");

	let dest_metadata = fs::metadata(&dest_file).await.unwrap();
	assert_eq!(
		dest_metadata.len(),
		expected_size,
		"Copied file size should match source"
	);

	// Calculate effective copy speed
	let total_time = start_time.elapsed();
	let mb_per_second = (file_size_mb as f64) / total_time.as_secs_f64();
	println!("\nCopy performance: {:.1} MB/s", mb_per_second);

	println!("\n✅ Copy progress monitoring test passed!");
	println!("   - Progress updated smoothly with byte-level granularity");
	println!("   - No large progress jumps detected");
	println!("   - File copied successfully with checksum verification");
}

#[tokio::test]
async fn test_copy_progress_multiple_files() {
	// Initialize tracing subscriber for debug logs
	let _guard = tracing_subscriber::registry()
		.with(tracing_subscriber::fmt::layer())
		.with(
			tracing_subscriber::EnvFilter::try_from_default_env()
				.unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
		)
		.try_init();

	// This test verifies progress tracking across multiple files
	let temp_dir = TempDir::new().unwrap();
	let test_root = temp_dir.path();

	let source_dir = test_root.join("source");
	let dest_dir = test_root.join("destination");
	fs::create_dir_all(&source_dir).await.unwrap();
	fs::create_dir_all(&dest_dir).await.unwrap();

	// Create 4 files of different sizes
	let files = vec![
		("file1.bin", 100), // 100MB
		("file2.bin", 200), // 200MB
		("file3.bin", 150), // 150MB
		("file4.bin", 50),  // 50MB
	];

	let mut source_files = Vec::new();
	for (name, size_mb) in &files {
		let path = source_dir.join(name);
		println!("Creating {} ({}MB)...", name, size_mb);
		create_large_test_file(&path, *size_mb).await.unwrap();
		source_files.push(path);
	}

	// Initialize core and library
	let core_data_dir = test_root.join("core_data");
	let core = Core::new_with_config(core_data_dir).await.unwrap();
	let library = core
		.libraries
		.create_library("Multi-file Progress Test", None, core.context.clone())
		.await
		.unwrap();
	let library_id = library.id();

	let context = core.context.clone();
	let action_manager = ActionManager::new(context);

	// Build copy action for multiple files
	let copy_action = FileCopyAction {
		sources: source_files,
		destination: dest_dir.clone(),
		options: CopyOptions {
			overwrite: false,
			verify_checksum: true,
			preserve_timestamps: true,
			delete_after_copy: false,
			move_mode: None,
			copy_method: CopyMethod::StreamingCopy,
		},
	};

	let action = Action::FileCopy {
		library_id,
		action: copy_action,
	};

	// Setup progress monitoring
	let progress_snapshots = Arc::new(Mutex::new(Vec::new()));
	let progress_snapshots_clone = progress_snapshots.clone();

	// Execute the action
	println!("\nStarting multi-file copy operation...");
	let action_output = action_manager
		.dispatch(action)
		.await
		.expect("Action dispatch should succeed");

	let job_id = match &action_output {
		sd_core_new::infrastructure::actions::output::ActionOutput::Custom { data, .. } => {
			let job_id_str = data.get("job_id").unwrap().as_str().unwrap();
			Uuid::parse_str(job_id_str).unwrap()
		}
		_ => panic!("Expected Custom ActionOutput variant"),
	};

	// Monitor progress
	let library_clone = library.clone();
	let monitor_handle = tokio::spawn(async move {
		let mut last_progress = 0.0;

		loop {
			let job_info_result = library_clone.jobs().get_job_info(job_id).await.unwrap();
			if let Some(job_info) = job_info_result {
				let current_progress = job_info.progress * 100.0;

				if (current_progress - last_progress).abs() > 0.01 {
					let snapshot = ProgressSnapshot {
						timestamp: std::time::Instant::now(),
						percentage: current_progress,
						bytes_copied: 0, // Would need to calculate from percentage
						message: format!("{:.1}%", current_progress),
					};

					println!("Multi-file progress: {:.1}%", current_progress);
					progress_snapshots_clone.lock().unwrap().push(snapshot);
					last_progress = current_progress;
				}

				if matches!(job_info.status, JobStatus::Completed) {
					break;
				} else if matches!(job_info.status, JobStatus::Failed) {
					panic!("Multi-file job failed!");
				}
			} else {
				println!(
					"Job info returned None for multi-file job {}. Job likely completed.",
					job_id
				);
				break;
			}

			tokio::time::sleep(Duration::from_millis(50)).await;
		}
	});

	timeout(Duration::from_secs(30), monitor_handle)
		.await
		.expect("Multi-file copy should complete within 30 seconds")
		.expect("Monitor task should succeed");

	// Analyze progress
	let snapshots = progress_snapshots.lock().unwrap();
	println!("\n=== Multi-file Progress Analysis ===");
	println!("Total snapshots: {}", snapshots.len());

	// With 4 files totaling 500MB, we should see smooth progress
	// not 4 discrete 25% jumps
	let mut increments = Vec::new();
	for i in 1..snapshots.len() {
		let increment = snapshots[i].percentage - snapshots[i - 1].percentage;
		if increment > 0.0 {
			increments.push(increment);
		}
	}

	let max_increment = increments.iter().cloned().fold(0.0f32, f32::max);
	println!("Maximum progress increment: {:.2}%", max_increment);

	// Should have smooth progress, not 25% jumps
	assert!(
		max_increment < 15.0,
		"Progress should update smoothly across files, not jump by {:.1}%",
		max_increment
	);

	println!("\n✅ Multi-file progress monitoring test passed!");
}
