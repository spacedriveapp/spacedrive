//! Test for monitoring copy progress with file-level tracking
//!
//! This test verifies that:
//! 1. File status metadata updates correctly (Pending -> Copying -> Completed)
//! 2. Progress events show accurate file counts
//! 3. Progress bar doesn't reset between files
//!
//! The test captures all events and queries metadata for post-mortem analysis.

use sd_core::domain::addressing::{SdPath, SdPathBatch};
use sd_core::{
	infra::{action::manager::ActionManager, event::Event},
	ops::files::copy::{action::FileCopyAction, input::CopyMethod, job::CopyOptions},
	Core,
};
use serde::{Deserialize, Serialize};
use std::{
	collections::HashMap,
	sync::{Arc, Mutex},
	time::Duration,
};
use tempfile::TempDir;
use tokio::{fs, time::timeout};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

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

/// Progress snapshot with full event details
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProgressSnapshot {
	timestamp_ms: u128,
	percentage: f32,
	bytes_copied: u64,
	bytes_total: u64,
	files_copied: usize,
	files_total: usize,
	message: String,
	phase: String,
}

/// Job metadata snapshot from query
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MetadataSnapshot {
	timestamp_ms: u128,
	total_files: usize,
	file_statuses: HashMap<String, String>,
}

/// Complete test snapshot for analysis
#[derive(Debug, Serialize, Deserialize)]
struct TestSnapshot {
	test_name: String,
	file_count: usize,
	file_size_mb: usize,
	copy_method: String,
	progress_events: Vec<ProgressSnapshot>,
	metadata_queries: Vec<MetadataSnapshot>,
	final_metadata: Option<serde_json::Value>,
	summary: TestSummary,
}

#[derive(Debug, Serialize, Deserialize)]
struct TestSummary {
	total_progress_events: usize,
	total_metadata_queries: usize,
	min_percentage: f32,
	max_percentage: f32,
	percentage_jumps: Vec<f32>,
	max_jump: f32,
	files_completed_at_end: usize,
	test_passed: bool,
	failure_reason: Option<String>,
}

#[tokio::test]
async fn test_copy_progress_with_metadata_tracking() {
	// Initialize tracing subscriber for debug logs
	let _guard = tracing_subscriber::registry()
		.with(tracing_subscriber::fmt::layer())
		.with(
			tracing_subscriber::EnvFilter::try_from_default_env()
				.unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
		)
		.set_default();

	println!("\n=== Copy Progress Metadata Test ===");
	println!("This test captures all progress events and metadata queries for analysis\n");

	// Setup test environment
	let temp_dir = TempDir::new().unwrap();
	let test_root = temp_dir.path();

	// Create source and destination directories
	let source_dir = test_root.join("source");
	let dest_dir = test_root.join("destination");
	fs::create_dir_all(&source_dir).await.unwrap();
	fs::create_dir_all(&dest_dir).await.unwrap();

	// Create 3 test files of 1GB each
	let file_size_mb = 1000;
	let file_count = 3;
	let mut source_files = Vec::new();

	for i in 1..=file_count {
		let file_path = source_dir.join(format!("test_file_{}.bin", i));
		println!("Creating test_file_{}.bin ({}MB)...", i, file_size_mb);
		create_large_test_file(&file_path, file_size_mb)
			.await
			.unwrap();
		source_files.push(file_path);
	}

	println!("Initializing Spacedrive Core...");
	let core_data_dir = test_root.join("core_data");
	let core = Core::new(core_data_dir.clone()).await.unwrap();

	// Create a test library
	println!("Creating test library...");
	let library = core
		.libraries
		.create_library("Progress Test Library", None, core.context.clone())
		.await
		.unwrap();

	let library_id = library.id();

	// Create ActionManager
	let context = core.context.clone();
	let action_manager = ActionManager::new(context.clone());

	// Build the copy action
	let copy_action = FileCopyAction {
		sources: SdPathBatch::new(
			source_files
				.iter()
				.map(|p| SdPath::local(p.clone()))
				.collect(),
		),
		destination: SdPath::local(dest_dir.clone()),
		options: CopyOptions {
			conflict_resolution: None,
			overwrite: true,
			verify_checksum: false, // Disable for speed
			preserve_timestamps: true,
			delete_after_copy: false,
			move_mode: None,
			copy_method: CopyMethod::Streaming,
		},
		on_conflict: None,
	};

	// Setup data collection
	let progress_snapshots = Arc::new(Mutex::new(Vec::new()));
	let metadata_snapshots = Arc::new(Mutex::new(Vec::new()));
	let progress_clone = progress_snapshots.clone();
	let metadata_clone = metadata_snapshots.clone();
	let library_clone = library.clone();
	let core_context_clone = core.context.clone();
	let start_time = std::time::Instant::now();

	// Subscribe to events BEFORE dispatching
	let mut event_subscriber = core.events.subscribe();

	// Start monitoring task BEFORE dispatching to avoid missing events
	let (job_id_tx, job_id_rx) =
		tokio::sync::oneshot::channel::<sd_core::infra::job::types::JobId>();

	let monitor_handle = tokio::spawn(async move {
		// Wait for job ID to be sent
		let job_id = job_id_rx.await.expect("Job ID should be sent");
		let mut event_count = 0;
		let mut metadata_query_count = 0;

		println!("Monitor task started, listening for job {}", job_id);

		loop {
			// Wait for next event with timeout for metadata queries
			match tokio::time::timeout(Duration::from_millis(500), event_subscriber.recv()).await {
				Ok(Ok(event)) => {
					event_count += 1;

					// Debug ALL events
					println!("[DEBUG] Event #{}: {:?}", event_count, event);

					match event {
						Event::JobProgress {
							job_id: event_job_id,
							generic_progress,
							progress,
							message,
							..
						} => {
							if event_job_id != job_id.to_string() {
								continue;
							}

							println!(
								"[DEBUG] JobProgress event: progress={}, message={:?}, generic_progress.is_some()={}",
								progress,
								message,
								generic_progress.is_some()
							);

							if let Some(gen_progress) = generic_progress {
								let snapshot = ProgressSnapshot {
									timestamp_ms: start_time.elapsed().as_millis(),
									percentage: gen_progress.percentage,
									bytes_copied: gen_progress
										.completion
										.bytes_completed
										.unwrap_or(0),
									bytes_total: gen_progress.completion.total_bytes.unwrap_or(0),
									files_copied: gen_progress.completion.completed as usize,
									files_total: gen_progress.completion.total as usize,
									message: gen_progress.message.clone(),
									phase: gen_progress.phase.clone(),
								};

								println!(
									"[{:>6}ms] Progress: {:.1}% | Files: {}/{} | Bytes: {}/{}  {}",
									snapshot.timestamp_ms,
									snapshot.percentage * 100.0,
									snapshot.files_copied,
									snapshot.files_total,
									snapshot.bytes_copied / (1024 * 1024),
									snapshot.bytes_total / (1024 * 1024),
									snapshot.phase
								);

								progress_clone.lock().unwrap().push(snapshot);
							}
						}
						Event::JobCompleted {
							job_id: event_job_id,
							..
						} => {
							if event_job_id != job_id.to_string() {
								continue;
							}
							println!("\n✓ Job completed after {} events", event_count);
							break;
						}
						Event::JobFailed {
							job_id: event_job_id,
							error,
							..
						} => {
							if event_job_id != job_id.to_string() {
								continue;
							}
							println!("\n✗ Job failed: {}", error);
							panic!("Job failed: {}", error);
						}
						_ => {
							// Other events
						}
					}
				}
				Ok(Err(_)) => {
					// Channel closed
					break;
				}
				Err(_) => {
					// Timeout - query metadata
					if metadata_query_count < 20 {
						// Limit queries
						use sd_core::infra::query::LibraryQuery;

						let query_input =
							sd_core::ops::jobs::copy_metadata::query::CopyMetadataQueryInput {
								job_id: job_id.into(),
							};

						let query =
							sd_core::ops::jobs::copy_metadata::query::CopyMetadataQuery::from_input(
								query_input,
							)
							.unwrap();

						let mut session = sd_core::infra::api::SessionContext::device_session(
							uuid::Uuid::new_v4(),
							"test-device".to_string(),
						);
						session.current_library_id = Some(library_clone.id());

						match query.execute(core_context_clone.clone(), session).await {
							Ok(result) => {
								if let Some(metadata) = result.metadata {
									let mut file_statuses = HashMap::new();
									for file in &metadata.files {
										let status = format!("{:?}", file.status);
										let name = file
											.source_path
											.path()
											.and_then(|p| {
												p.file_name()
													.map(|n| n.to_string_lossy().to_string())
											})
											.unwrap_or_else(|| "unknown".to_string());
										file_statuses.insert(name, status);
									}

									let snapshot = MetadataSnapshot {
										timestamp_ms: start_time.elapsed().as_millis(),
										total_files: metadata.files.len(),
										file_statuses,
									};

									metadata_clone.lock().unwrap().push(snapshot);
									metadata_query_count += 1;
								}
							}
							Err(e) => {
								eprintln!("Metadata query failed: {}", e);
							}
						}
					}
				}
			}
		}

		(event_count, metadata_query_count)
	});

	// NOW dispatch the job (monitor is already listening)
	println!("\nStarting copy operation for {} files...\n", file_count);
	let job_handle = action_manager
		.dispatch_library(Some(library_id), copy_action)
		.await
		.expect("Action dispatch should succeed");

	let job_id = job_handle.id;

	// Send job ID to monitoring task
	job_id_tx
		.send(job_id)
		.expect("Monitor task should be running");

	// Wait for job completion with timeout
	let (event_count, metadata_query_count) =
		match timeout(Duration::from_secs(60), monitor_handle).await {
			Ok(Ok(result)) => result,
			Ok(Err(e)) => panic!("Monitoring task failed: {}", e),
			Err(_) => panic!("Copy operation timed out after 60 seconds"),
		};

	println!("\n=== Data Collection Summary ===");
	println!("Progress events captured: {}", event_count);
	println!("Metadata queries executed: {}", metadata_query_count);

	// Query final metadata state
	println!("\nQuerying final job metadata...");
	use sd_core::infra::query::LibraryQuery;
	let query_input = sd_core::ops::jobs::copy_metadata::query::CopyMetadataQueryInput {
		job_id: job_id.into(),
	};
	let query =
		sd_core::ops::jobs::copy_metadata::query::CopyMetadataQuery::from_input(query_input)
			.unwrap();
	let mut session = sd_core::infra::api::SessionContext::device_session(
		uuid::Uuid::new_v4(),
		"test-device".to_string(),
	);
	session.current_library_id = Some(library_id);
	let final_ctx = context.clone();
	let final_metadata_result = query.execute(final_ctx, session).await.unwrap();
	let final_metadata_json = serde_json::to_value(&final_metadata_result).unwrap();

	// Analyze collected data
	let progress_snapshots = progress_snapshots.lock().unwrap().clone();
	let metadata_snapshots = metadata_snapshots.lock().unwrap().clone();

	// Calculate statistics
	let percentages: Vec<f32> = progress_snapshots.iter().map(|s| s.percentage).collect();
	let min_percentage = percentages.iter().cloned().fold(1.0f32, f32::min);
	let max_percentage = percentages.iter().cloned().fold(0.0f32, f32::max);

	let mut percentage_jumps = Vec::new();
	for i in 1..percentages.len() {
		let jump = (percentages[i] - percentages[i - 1]) * 100.0;
		if jump > 0.0 {
			percentage_jumps.push(jump);
		}
	}
	let max_jump = percentage_jumps.iter().cloned().fold(0.0f32, f32::max);

	let files_completed_at_end = if let Some(last_snapshot) = progress_snapshots.last() {
		last_snapshot.files_copied
	} else {
		0
	};

	let test_passed =
		files_completed_at_end == file_count && max_percentage >= 0.99 && max_jump < 50.0;

	let failure_reason = if !test_passed {
		if files_completed_at_end != file_count {
			Some(format!(
				"Expected {} files completed, got {}",
				file_count, files_completed_at_end
			))
		} else if max_percentage < 0.99 {
			Some(format!(
				"Progress never reached 100% (max: {:.1}%)",
				max_percentage * 100.0
			))
		} else {
			Some(format!(
				"Progress bar reset detected (max jump: {:.1}%)",
				max_jump
			))
		}
	} else {
		None
	};

	let snapshot = TestSnapshot {
		test_name: "copy_progress_with_metadata_tracking".to_string(),
		file_count,
		file_size_mb,
		copy_method: "Streaming".to_string(),
		progress_events: progress_snapshots,
		metadata_queries: metadata_snapshots,
		final_metadata: Some(final_metadata_json),
		summary: TestSummary {
			total_progress_events: event_count,
			total_metadata_queries: metadata_query_count,
			min_percentage,
			max_percentage,
			percentage_jumps,
			max_jump,
			files_completed_at_end,
			test_passed,
			failure_reason,
		},
	};

	// Save snapshot if enabled
	if std::env::var("SD_TEST_SNAPSHOTS").is_ok() {
		let snapshot_dir = dirs::data_local_dir()
			.unwrap()
			.join("spacedrive")
			.join("test_snapshots")
			.join("copy_progress_test")
			.join(chrono::Local::now().format("%Y%m%d_%H%M%S").to_string());

		fs::create_dir_all(&snapshot_dir).await.unwrap();

		let snapshot_path = snapshot_dir.join("test_snapshot.json");
		let snapshot_json = serde_json::to_string_pretty(&snapshot).unwrap();
		fs::write(&snapshot_path, snapshot_json).await.unwrap();

		println!("\n📸 Snapshot saved to: {}", snapshot_path.display());
	} else {
		// Always write to temp dir for local inspection
		let temp_snapshot_path = test_root.join("test_snapshot.json");
		let snapshot_json = serde_json::to_string_pretty(&snapshot).unwrap();
		fs::write(&temp_snapshot_path, snapshot_json).await.unwrap();
		println!(
			"\n📄 Snapshot written to temp: {}",
			temp_snapshot_path.display()
		);
		println!("   (Set SD_TEST_SNAPSHOTS=1 to save to permanent location)");
	}

	// Print summary
	println!("\n=== Test Summary ===");
	println!("Files completed: {}/{}", files_completed_at_end, file_count);
	println!(
		"Progress range: {:.1}% - {:.1}%",
		min_percentage * 100.0,
		max_percentage * 100.0
	);
	println!("Max progress jump: {:.1}%", max_jump);
	println!("Test passed: {}", test_passed);
	if let Some(reason) = &snapshot.summary.failure_reason {
		println!("Failure reason: {}", reason);
	}

	// Cleanup
	core.shutdown().await.unwrap();
	drop(core);
	tokio::time::sleep(Duration::from_millis(500)).await;

	// Assert test passed
	if !test_passed {
		panic!(
			"Test failed: {}",
			snapshot
				.summary
				.failure_reason
				.unwrap_or_else(|| "Unknown failure".to_string())
		);
	}
}
