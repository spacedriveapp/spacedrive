//! Test for monitoring copy progress with large files
//!
//! This test verifies that copy progress updates smoothly with byte-level
//! granularity rather than jumping in large increments.

use sd_core::domain::addressing::{SdPath, SdPathBatch};
use sd_core::{
	infra::{action::manager::ActionManager, event::Event},
	ops::files::copy::{action::FileCopyAction, input::CopyMethod, job::CopyOptions},
	Core,
};
use std::{
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
		sources: SdPathBatch::new(vec![SdPath::local(source_file.clone())]),
		destination: SdPath::local(dest_dir.clone()),
		options: CopyOptions {
			overwrite: false,
			verify_checksum: true,     // --verify
			preserve_timestamps: true, // --preserve-timestamps
			delete_after_copy: false,
			move_mode: None,
			copy_method: CopyMethod::StreamingCopy, // --method streaming
		},
	};

	// Dispatch the action directly via ActionManager (library-scoped)

	// Setup progress monitoring
	let progress_snapshots = Arc::new(Mutex::new(Vec::new()));
	let progress_snapshots_clone = progress_snapshots.clone();
	let start_time = std::time::Instant::now();

	// Execute the action
	println!("Starting copy operation...");
	let _job_handle = action_manager
		.dispatch_library(library_id, copy_action)
		.await
		.expect("Action dispatch should succeed");

	// Job ID will be read from first Job* event below

	// Subscribe to events from the event bus
	let mut event_subscriber = core.events.subscribe();
	let expected_size_clone = expected_size;
	let mut observed_job_id: Option<String> = None;

	// Start monitoring task using EventBus
	let monitor_handle = tokio::spawn(async move {
		let mut last_progress = 0.0;
		let mut has_seen_progress = false;
		let mut event_count = 0;

		while let Ok(event) = event_subscriber.recv().await {
			event_count += 1;

			match event {
				Event::JobProgress {
					job_id: event_job_id,
					progress,
					message,
					..
				} => {
					if observed_job_id.is_none() {
						observed_job_id = Some(event_job_id.clone());
					}
					if let Some(ref jid) = observed_job_id {
						if &event_job_id != jid {
							continue;
						}
					}
					let current_progress = progress * 100.0;

					// Record snapshot if progress changed
					if (current_progress - last_progress).abs() > 0.01 {
						has_seen_progress = true;

						let snapshot = ProgressSnapshot {
							timestamp: std::time::Instant::now(),
							percentage: current_progress as f32,
							bytes_copied: (expected_size_clone as f64 * (progress as f64)) as u64,
							message: message.unwrap_or_else(|| format!("{:.1}%", current_progress)),
						};

						println!(
							"Progress: {:.1}% ({} MB)",
							current_progress,
							snapshot.bytes_copied / (1024 * 1024)
						);

						progress_snapshots_clone.lock().unwrap().push(snapshot);
						last_progress = current_progress;
					}
				}
				Event::JobCompleted {
					job_id: event_job_id,
					..
				} => {
					if let Some(ref jid) = observed_job_id {
						if &event_job_id != jid {
							continue;
						}
					} else {
						observed_job_id = Some(event_job_id.clone());
					}
					println!("Job completed! (after {} events)", event_count);
					println!("Final progress: {:.1}%", last_progress);

					// Record final progress if we haven't seen any updates
					if !has_seen_progress && last_progress == 0.0 {
						let snapshot = ProgressSnapshot {
							timestamp: std::time::Instant::now(),
							percentage: 100.0,
							bytes_copied: expected_size_clone,
							message: "Final".to_string(),
						};
						progress_snapshots_clone.lock().unwrap().push(snapshot);
					}
					break;
				}
				Event::JobFailed {
					job_id: event_job_id,
					error,
					..
				} => {
					if let Some(ref jid) = observed_job_id {
						if &event_job_id != jid {
							continue;
						}
					} else {
						observed_job_id = Some(event_job_id.clone());
					}
					println!("Job failed after {} events: {}", event_count, error);
					panic!("Job failed: {}", error);
				}
				Event::JobCancelled {
					job_id: event_job_id,
					..
				} => {
					if let Some(ref jid) = observed_job_id {
						if &event_job_id != jid {
							continue;
						}
					} else {
						observed_job_id = Some(event_job_id.clone());
					}
					println!("Job was cancelled after {} events", event_count);
					break;
				}
				_ => {
					// Other events - continue monitoring
				}
			}
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
		sources: SdPathBatch::new(source_files.iter().cloned().map(SdPath::local).collect()),
		destination: SdPath::local(dest_dir.clone()),
		options: CopyOptions {
			overwrite: false,
			verify_checksum: true,
			preserve_timestamps: true,
			delete_after_copy: false,
			move_mode: None,
			copy_method: CopyMethod::StreamingCopy,
		},
	};

	// Dispatch the action directly via ActionManager (library-scoped)

	// Setup progress monitoring
	let progress_snapshots = Arc::new(Mutex::new(Vec::new()));
	let progress_snapshots_clone = progress_snapshots.clone();

	// Execute the action
	println!("\nStarting multi-file copy operation...");
	let _job_handle = action_manager
		.dispatch_library(library_id, copy_action)
		.await
		.expect("Action dispatch should succeed");

	// Subscribe to events and monitor progress using EventBus
	let mut event_subscriber = core.events.subscribe();
	let mut observed_job_id: Option<String> = None;

	let monitor_handle = tokio::spawn(async move {
		let mut last_progress = 0.0;

		while let Ok(event) = event_subscriber.recv().await {
			match event {
				Event::JobProgress {
					job_id: event_job_id,
					progress,
					..
				} => {
					if observed_job_id.is_none() {
						observed_job_id = Some(event_job_id.clone());
					}
					if let Some(ref jid) = observed_job_id {
						if &event_job_id != jid {
							continue;
						}
					}
					let current_progress = progress * 100.0;

					if (current_progress - last_progress).abs() > 0.01 {
						let snapshot = ProgressSnapshot {
							timestamp: std::time::Instant::now(),
							percentage: current_progress as f32,
							bytes_copied: 0, // Would need to calculate from percentage
							message: format!("{:.1}%", current_progress),
						};

						println!("Multi-file progress: {:.1}%", current_progress);
						progress_snapshots_clone.lock().unwrap().push(snapshot);
						last_progress = current_progress;
					}
				}
				Event::JobCompleted {
					job_id: event_job_id,
					..
				} => {
					if let Some(ref jid) = observed_job_id {
						if &event_job_id != jid {
							continue;
						}
					} else {
						observed_job_id = Some(event_job_id.clone());
					}
					println!("Multi-file job completed");
					break;
				}
				Event::JobFailed {
					job_id: event_job_id,
					error,
					..
				} => {
					if let Some(ref jid) = observed_job_id {
						if &event_job_id != jid {
							continue;
						}
					} else {
						observed_job_id = Some(event_job_id.clone());
					}
					panic!("Multi-file job failed: {}", error);
				}
				Event::JobCancelled {
					job_id: event_job_id,
					..
				} => {
					if let Some(ref jid) = observed_job_id {
						if &event_job_id != jid {
							continue;
						}
					} else {
						observed_job_id = Some(event_job_id.clone());
					}
					println!("Multi-file job was cancelled");
					break;
				}
				_ => {
					// Other events - continue monitoring
				}
			}
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
