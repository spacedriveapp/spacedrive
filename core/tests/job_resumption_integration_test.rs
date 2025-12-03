//! Integration test for job resumption at various interruption points
//!
//! This test generates benchmark data and tests job resumption by interrupting
//! indexing jobs at different phases and progress points, then verifying they
//! can resume and complete successfully.

use sd_core::{
	domain::SdPath,
	infra::action::LibraryAction,
	ops::{
		indexing::IndexMode,
		locations::add::action::{LocationAddAction, LocationAddInput},
	},
	testing::integration_utils::IntegrationTestSetup,
};
use std::{
	path::PathBuf,
	sync::{
		atomic::{AtomicBool, AtomicU32, Ordering},
		Arc,
	},
	time::Duration,
};
use tokio::{
	sync::mpsc,
	time::{sleep, timeout},
};
use tracing::{info, warn};
use uuid::Uuid;

/// Benchmark recipe name to use for test data generation
/// Using existing generated data from desktop_complex (or fallback to shape_medium if available)
const TEST_RECIPE_NAME: &str = "desktop_complex";

/// Path where the benchmark data will be generated (relative to workspace root)
/// Will check for desktop_complex first, then fallback to shape_medium if it exists
const TEST_INDEXING_DATA_PATH: &str = "core/benchdata/desktop_complex";

/// Different interruption points to test
#[derive(Debug, Clone)]
enum InterruptionPoint {
	/// Interrupt during discovery phase after N progress events
	DiscoveryAfterEvents(u32),
	/// Interrupt during processing phase after N progress events
	ProcessingAfterEvents(u32),
	/// Interrupt during content identification after N progress events
	ContentIdentificationAfterEvents(u32),
	/// Interrupt during aggregation phase (immediately when detected)
	Aggregation,
}

/// Test result for a single interruption scenario
#[derive(Debug)]
struct TestResult {
	interruption_point: InterruptionPoint,
	success: bool,
	error: Option<String>,
	job_log_path: Option<PathBuf>,
	test_log_path: Option<PathBuf>,
}

/// Main integration test for job resumption with realistic desktop-scale data
///
/// This test uses the desktop_complex recipe (500k files, 8 levels deep) to simulate
/// real-world indexing scenarios where jobs take 5+ minutes and users may interrupt
/// at any point. Each phase should generate many progress events, allowing us to test
/// interruption and resumption at various points within each phase.
///
/// Expected behavior:
/// - Discovery: Should generate 50+ progress events with 500k files across deep directories
/// - Processing: Should generate 100+ progress events while processing file metadata
/// - Content Identification: Should generate 500+ progress events while hashing files
/// - Each interrupted job should cleanly pause and resume from where it left off
#[tokio::test]
async fn test_job_resumption_at_various_points() {
	info!("Starting job resumption integration test");

	// Generate benchmark data (or use existing data)
	info!("Preparing test data");
	let indexing_data_path = generate_test_data()
		.await
		.expect("Failed to prepare test data");

	// Define interruption points to test with realistic event counts for smaller datasets
	// For Downloads folder, use lower event counts since there are fewer files
	let interruption_points = vec![
		InterruptionPoint::DiscoveryAfterEvents(2), // Interrupt early in discovery
		InterruptionPoint::ProcessingAfterEvents(2), // Interrupt early in processing
		InterruptionPoint::ContentIdentificationAfterEvents(2), // Interrupt early in content ID
	];

	let mut results = Vec::new();
	let total_tests = interruption_points.len();

	// Test each interruption point
	for (i, interruption_point) in interruption_points.into_iter().enumerate() {
		info!(
			"Testing interruption point {:?} ({}/{})",
			interruption_point,
			i + 1,
			total_tests
		);

		let result =
			test_single_interruption_point(&indexing_data_path, interruption_point.clone(), i)
				.await;

		results.push(result);

		// Brief pause between tests
		sleep(Duration::from_secs(2)).await;
	}

	// Analyze results
	analyze_test_results(&results);

	// Assert all tests passed
	let failed_tests: Vec<_> = results.iter().filter(|r| !r.success).collect();
	if !failed_tests.is_empty() {
		panic!(
			"Job resumption test failed at {} interruption points: {:#?}",
			failed_tests.len(),
			failed_tests
		);
	}

	info!("All job resumption tests passed! ");
	info!("Test logs and data available in: test_data/");
}

/// Generate test data using benchmark data generation
async fn generate_test_data() -> Result<PathBuf, Box<dyn std::error::Error>> {
	// Use Downloads folder instead of benchmark data
	let home_dir = std::env::var("HOME")
		.map(PathBuf::from)
		.or_else(|_| std::env::current_dir())?;

	let indexing_data_path = home_dir.join("Downloads");

	if !indexing_data_path.exists() {
		return Err(format!(
			"Downloads folder does not exist at: {}",
			indexing_data_path.display()
		)
		.into());
	}

	info!(
		"Using Downloads folder at: {}",
		indexing_data_path.display()
	);
	Ok(indexing_data_path)
}

/// Test a single interruption point scenario
async fn test_single_interruption_point(
	indexing_data_path: &PathBuf,
	interruption_point: InterruptionPoint,
	test_index: usize,
) -> TestResult {
	let test_name = format!("test_{:02}_{:?}", test_index, interruption_point);

	// Create test environment with custom tracing
	let test_setup = match IntegrationTestSetup::with_tracing(
		&test_name,
		"warn,sd_core=info,job_resumption_integration_test=info",
	)
	.await
	{
		Ok(setup) => setup,
		Err(error) => {
			return TestResult {
				interruption_point,
				success: false,
				error: Some(format!("Failed to create test setup: {}", error)),
				job_log_path: None,
				test_log_path: None,
			};
		}
	};

	info!(
		"Testing {} with data at {}",
		test_name,
		indexing_data_path.display()
	);

	// Phase 1: Start indexing and interrupt at specified point
	let interrupt_result =
		start_and_interrupt_job(&test_setup, indexing_data_path, &interruption_point).await;

	let (job_id, library_id) = match interrupt_result {
		Ok(result) => result,
		Err(error) => {
			return TestResult {
				interruption_point,
				success: false,
				error: Some(format!("Failed to interrupt job: {}", error)),
				job_log_path: None,
				test_log_path: None,
			};
		}
	};

	// Clean up SQLite lock files to ensure clean restart
	info!("Cleaning up database lock files...");
	let library_dir = test_setup.data_dir().join("libraries");
	if library_dir.exists() {
		for entry in std::fs::read_dir(&library_dir)? {
			let entry = entry?;
			let path = entry.path();
			if path.is_dir() {
				// Remove SQLite WAL and SHM files
				let db_path = path.join("library.db");
				let wal_path = path.join("library.db-wal");
				let shm_path = path.join("library.db-shm");

				for lock_file in [wal_path, shm_path] {
					if lock_file.exists() {
						if let Err(e) = std::fs::remove_file(&lock_file) {
							warn!("Failed to remove {}: {}", lock_file.display(), e);
						}
					}
				}
			}
		}
	}

	// Longer pause to ensure all database locks are released
	info!("Waiting for database locks to release...");
	sleep(Duration::from_secs(2)).await;

	// Phase 2: Resume and complete the job
	let resume_result =
		resume_and_complete_job(&test_setup, indexing_data_path, job_id, library_id).await;

	match resume_result {
		Ok((job_log_path, test_log_path)) => TestResult {
			interruption_point,
			success: true,
			error: None,
			job_log_path: Some(job_log_path),
			test_log_path: Some(test_log_path),
		},
		Err(error) => TestResult {
			interruption_point,
			success: false,
			error: Some(format!("Failed to resume job: {}", error)),
			job_log_path: None,
			test_log_path: None,
		},
	}
}

/// Start indexing job and interrupt at specified point
async fn start_and_interrupt_job(
	test_setup: &IntegrationTestSetup,
	indexing_data_path: &PathBuf,
	interruption_point: &InterruptionPoint,
) -> Result<(Uuid, Uuid), Box<dyn std::error::Error>> {
	info!(
		"Starting job and waiting for interruption point: {:?}",
		interruption_point
	);

	// Create core using the test setup's configuration
	let core = test_setup.create_core().await?;
	let core_context = core.context.clone();

	// Create library
	let library = core_context
		.libraries()
		.await
		.create_library("Test Library".to_string(), None, core_context.clone())
		.await?;

	let library_id = library.id();

	// Create location add action to automatically trigger indexing
	let location_input = LocationAddInput {
		path: SdPath::local(indexing_data_path.clone()),
		name: Some("Test Location".to_string()),
		mode: IndexMode::Content,
		job_policies: None,
	};

	let location_action = LocationAddAction::from_input(location_input)
		.map_err(|e| format!("Failed to create location action: {}", e))?;

	// Dispatch the location add action through the action manager
	let action_manager = core_context.action_manager.read().await;
	let action_manager = action_manager
		.as_ref()
		.ok_or("Action manager not initialized")?;

	let location_output = action_manager
		.dispatch_library(Some(library.id()), location_action)
		.await
		.map_err(|e| format!("Failed to dispatch location add action: {}", e))?;

	// The location add action automatically creates an indexing job
	let job_id = location_output
		.job_id
		.ok_or("Location add action did not return a job ID")?;

	// Set up event monitoring
	let (interrupt_tx, mut interrupt_rx) = mpsc::channel(1);
	let should_interrupt = Arc::new(AtomicBool::new(false));
	let should_interrupt_clone = should_interrupt.clone();
	let phase_order_failed = Arc::new(AtomicBool::new(false));
	let phase_order_failed_clone = phase_order_failed.clone();

	// Monitor events for interruption point
	let mut event_rx = core_context.events.subscribe();
	let interruption_point_clone = interruption_point.clone();

	// Event counters for each phase
	let discovery_events = Arc::new(AtomicU32::new(0));
	let processing_events = Arc::new(AtomicU32::new(0));
	let content_events = Arc::new(AtomicU32::new(0));

	let discovery_events_clone = discovery_events.clone();
	let processing_events_clone = processing_events.clone();
	let content_events_clone = content_events.clone();

	tokio::spawn(async move {
		while let Ok(event) = event_rx.recv().await {
			if let sd_core::infra::event::Event::JobProgress {
				job_id: event_job_id,
				progress: _,
				message,
				generic_progress,
				..
			} = event
			{
				if event_job_id == job_id.to_string() {
					let message_str = message.as_deref().unwrap_or("");

					// Extract phase from generic_progress if available
					let phase_name = if let Some(gp_value) = &generic_progress {
						if let Ok(gp_json) = serde_json::to_value(gp_value) {
							gp_json
								.get("phase")
								.and_then(|p| p.as_str())
								.map(|s| s.to_string())
								.unwrap_or_default()
						} else {
							String::new()
						}
					} else {
						String::new()
					};

					info!("Job progress: {} - {}", phase_name, message_str);

					// Check if we've moved past our target phase (test failure condition)
					let phase_order_failed = match &interruption_point_clone {
						InterruptionPoint::DiscoveryAfterEvents(_) => {
							// If we're targeting Discovery but see Processing/Content/Aggregation, we failed
							phase_name == "Processing"
								|| phase_name == "Content Identification"
								|| phase_name == "Finalizing"
						}
						InterruptionPoint::ProcessingAfterEvents(_) => {
							// If we're targeting Processing but see Content/Aggregation, we failed
							phase_name == "Content Identification" || phase_name == "Finalizing"
						}
						InterruptionPoint::ContentIdentificationAfterEvents(_) => {
							// If we're targeting Content but see Finalizing, we failed
							phase_name == "Finalizing"
						}
						InterruptionPoint::Aggregation => {
							// Aggregation is the last phase, no failure condition
							false
						}
					};

					if phase_order_failed && !should_interrupt_clone.load(Ordering::Relaxed) {
						warn!("TEST FAILURE: Reached phase '{}' before hitting interruption point {:?}!",
                              message_str, interruption_point_clone);
						phase_order_failed_clone.store(true, Ordering::Relaxed);
						should_interrupt_clone.store(true, Ordering::Relaxed);
						let _ = interrupt_tx.send(()).await;
						return; // Exit the event loop
					}

					// Count events for each phase using the actual phase name
					let (current_count, should_interrupt_now) = match &interruption_point_clone {
						InterruptionPoint::DiscoveryAfterEvents(target_count) => {
							if phase_name == "Discovery" {
								let count =
									discovery_events_clone.fetch_add(1, Ordering::Relaxed) + 1;
								info!("Discovery event #{}: {}", count, message_str);
								(count, count >= *target_count)
							} else {
								(0, false)
							}
						}
						InterruptionPoint::ProcessingAfterEvents(target_count) => {
							if phase_name == "Processing" {
								let count =
									processing_events_clone.fetch_add(1, Ordering::Relaxed) + 1;
								info!("Processing event #{}: {}", count, message_str);
								(count, count >= *target_count)
							} else {
								(0, false)
							}
						}
						InterruptionPoint::ContentIdentificationAfterEvents(target_count) => {
							if phase_name == "Content Identification" {
								let count =
									content_events_clone.fetch_add(1, Ordering::Relaxed) + 1;
								info!("Content identification event #{}: {}", count, message_str);
								(count, count >= *target_count)
							} else {
								(0, false)
							}
						}
						InterruptionPoint::Aggregation => {
							// Interrupt as soon as we enter aggregation phase
							(0, phase_name == "Aggregation")
						}
					};

					if should_interrupt_now && !should_interrupt_clone.load(Ordering::Relaxed) {
						info!(
							"Triggering interrupt after {} events in phase: {}",
							current_count, message_str
						);
						should_interrupt_clone.store(true, Ordering::Relaxed);
						let _ = interrupt_tx.send(()).await;
					}
				}
			}
		}
	});

	// Wait for interruption point or timeout
	let interrupt_timeout = timeout(Duration::from_secs(30), interrupt_rx.recv()).await;

	match interrupt_timeout {
		Ok(Some(())) => {
			// Check if this was a phase order failure
			if phase_order_failed.load(Ordering::Relaxed) {
				// Force kill the core immediately
				core.shutdown().await?;

				// Delete the redb database to release file locks (test workaround)
				let secrets_db_path = test_setup.data_dir().join("secrets.redb");
				if secrets_db_path.exists() {
					if let Err(e) = tokio::fs::remove_file(&secrets_db_path).await {
						warn!("Failed to remove secrets database: {}", e);
					}
				}

				sleep(Duration::from_millis(200)).await;

				Err(
					"Phase order failure: reached a later phase before hitting interruption point"
						.into(),
				)
			} else {
				info!("Interruption point reached, shutting down core");
				let result_job_id = job_id;

				// Shutdown core gracefully
				core.shutdown().await?;

				// Delete the redb database to release file locks (test workaround)
				let secrets_db_path = test_setup.data_dir().join("secrets.redb");
				if secrets_db_path.exists() {
					if let Err(e) = tokio::fs::remove_file(&secrets_db_path).await {
						warn!("Failed to remove secrets database: {}", e);
					}
				}

				// Brief delay to ensure cleanup
				sleep(Duration::from_millis(200)).await;

				Ok((result_job_id, library_id))
			}
		}
		Ok(None) => Err("Interrupt channel closed unexpectedly".into()),
		Err(_) => Err("Timeout waiting for interruption point".into()),
	}
}

/// Resume and complete the interrupted job
async fn resume_and_complete_job(
	test_setup: &IntegrationTestSetup,
	_indexing_data_path: &PathBuf,
	job_id: Uuid,
	library_id: Uuid,
) -> Result<(PathBuf, PathBuf), Box<dyn std::error::Error>> {
	info!("Resuming job {} and waiting for completion", job_id);

	// Create core again (simulating daemon restart)
	let core = test_setup.create_core().await?;
	let core_context = core.context.clone();

	// Get the specific library by ID (don't rely on auto-load which may fail due to locks)
	let libraries = core_context.libraries().await.list().await;
	let library = libraries
		.iter()
		.find(|lib| lib.id() == library_id)
		.ok_or_else(|| {
			format!(
				"Library {} not found after restart. Found {} libraries",
				library_id,
				libraries.len()
			)
		})?;

	// Check job status immediately after core initialization
	// Jobs may have already completed during the core startup process
	info!("Checking initial job status for job {}", job_id);
	let job_manager = library.jobs();

	// Check if job is already completed
	if let Ok(Some(job_info)) = job_manager.get_job_info(job_id).await {
		let job_status = job_info.status;

		info!("Job {} current status: {:?}", job_id, job_status);

		match job_status {
			sd_core::infra::job::types::JobStatus::Completed => {
				info!(
					"Job {} already completed during startup, no need to wait for events",
					job_id
				);

				// Collect log paths for inspection
				let job_log_path = test_setup.env().job_log_path(job_id);
				let test_log_path = test_setup
					.env()
					.log_file_path(&format!("{}.log", test_setup.env().test_name));

				// Shutdown core
				core.shutdown().await?;

				return Ok((job_log_path, test_log_path));
			}
			sd_core::infra::job::types::JobStatus::Failed => {
				core.shutdown().await?;
				return Err(format!("Job {} failed during startup", job_id).into());
			}
			_ => {
				info!(
					"Job {} is still running (status: {:?}), will monitor for completion",
					job_id, job_status
				);
			}
		}
	} else {
		warn!(
			"Could not get job info for job {}, will monitor for completion events",
			job_id
		);
	}

	// Set up completion monitoring
	let (completion_tx, mut completion_rx) = mpsc::channel(1);
	let job_completed = Arc::new(AtomicBool::new(false));
	let job_completed_clone = job_completed.clone();

	// Track last event time for timeout detection
	let last_event_time = Arc::new(std::sync::Mutex::new(std::time::Instant::now()));
	let last_event_time_clone = last_event_time.clone();

	// Clone completion_tx for the timeout monitor before moving into async block
	let completion_tx_timeout = completion_tx.clone();

	// Monitor for job completion and progress events
	let mut event_rx = core_context.events.subscribe();
	tokio::spawn(async move {
		while let Ok(event) = event_rx.recv().await {
			match event {
				sd_core::infra::event::Event::JobCompleted {
					job_id: event_job_id,
					..
				} => {
					if event_job_id == job_id.to_string() {
						info!("Job {} completed successfully", job_id);
						job_completed_clone.store(true, Ordering::Relaxed);
						let _ = completion_tx.send(Ok(())).await;
						break;
					}
				}
				sd_core::infra::event::Event::JobFailed {
					job_id: event_job_id,
					error,
					..
				} => {
					if event_job_id == job_id.to_string() {
						warn!("Job {} failed: {}", job_id, error);
						let _ = completion_tx.send(Err(error)).await;
						break;
					}
				}
				sd_core::infra::event::Event::JobProgress {
					job_id: event_job_id,
					message,
					generic_progress,
					..
				} => {
					if event_job_id == job_id.to_string() {
						// Update last event time when we receive progress events
						if let Ok(mut last_time) = last_event_time_clone.lock() {
							*last_time = std::time::Instant::now();
						}

						let message_str = message.as_deref().unwrap_or("");

						// Extract phase from generic_progress if available
						let phase_name = if let Some(gp_value) = &generic_progress {
							if let Ok(gp_json) = serde_json::to_value(gp_value) {
								gp_json
									.get("phase")
									.and_then(|p| p.as_str())
									.map(|s| s.to_string())
									.unwrap_or_default()
							} else {
								String::new()
							}
						} else {
							String::new()
						};

						// Debug: Log all progress events during resume to see what we're getting
						info!("Job progress: {} - {}", phase_name, message_str);
					}
				}
				_ => {}
			}
		}
	});

	// Add a timeout monitor for detecting unresponsive jobs
	let last_event_time_monitor = last_event_time.clone();
	tokio::spawn(async move {
		loop {
			sleep(Duration::from_secs(1)).await; // Check every second

			let time_since_last_event = {
				if let Ok(last_time) = last_event_time_monitor.lock() {
					last_time.elapsed()
				} else {
					Duration::from_secs(0)
				}
			};

			// If no events received in 30 seconds, consider the job unresponsive
			// The Aggregation phase may not emit progress events frequently, so use a longer timeout
			if time_since_last_event >= Duration::from_secs(30) {
				warn!(
					"Job {} appears unresponsive - no progress events received in {} seconds",
					job_id,
					time_since_last_event.as_secs()
				);
				let _ = completion_tx_timeout
					.send(Err(format!(
						"Job became unresponsive - no progress events received in {} seconds",
						time_since_last_event.as_secs()
					)))
					.await;
				break;
			}
		}
	});

	// Wait for completion or timeout (increased for large dataset)
	// Resume phase may need to process remaining files, allow generous time
	let completion_timeout = timeout(Duration::from_secs(900), completion_rx.recv()).await;

	match completion_timeout {
		Ok(Some(Ok(()))) => {
			info!("Job completed successfully");

			// Collect log paths for inspection
			let job_log_path = test_setup.env().job_log_path(job_id);
			let test_log_path = test_setup
				.env()
				.log_file_path(&format!("{}.log", test_setup.env().test_name));

			// Shutdown core
			core.shutdown().await?;

			Ok((job_log_path, test_log_path))
		}
		Ok(Some(Err(error))) => {
			core.shutdown().await?;
			Err(format!("Job failed during resumption: {}", error).into())
		}
		Ok(None) => {
			core.shutdown().await?;
			Err("Completion channel closed unexpectedly".into())
		}
		Err(_) => {
			core.shutdown().await?;
			Err("Timeout waiting for job completion".into())
		}
	}
}

/// Analyze and report test results
fn analyze_test_results(results: &[TestResult]) {
	info!("=== Job Resumption Test Results ===");

	let total_tests = results.len();
	let passed_tests = results.iter().filter(|r| r.success).count();
	let failed_tests = total_tests - passed_tests;

	info!("Total tests: {}", total_tests);
	info!("Passed: {}", passed_tests);
	info!("Failed: {}", failed_tests);

	if failed_tests > 0 {
		warn!("Failed test details:");
		for result in results.iter().filter(|r| !r.success) {
			warn!(
				"  {:?}: {}",
				result.interruption_point,
				result
					.error
					.as_ref()
					.unwrap_or(&"Unknown error".to_string())
			);

			if let Some(job_log) = &result.job_log_path {
				warn!("    Job log: {}", job_log.display());
			}
			if let Some(test_log) = &result.test_log_path {
				warn!("    Test log: {}", test_log.display());
			}
		}
	}

	// Group results by interruption type
	let mut by_phase = std::collections::HashMap::new();
	for result in results {
		let phase = match &result.interruption_point {
			InterruptionPoint::DiscoveryAfterEvents(_) => "Discovery",
			InterruptionPoint::ProcessingAfterEvents(_) => "Processing",
			InterruptionPoint::ContentIdentificationAfterEvents(_) => "Content Identification",
			InterruptionPoint::Aggregation => "Aggregation",
		};
		by_phase.entry(phase).or_insert_with(Vec::new).push(result);
	}

	info!("Results by phase:");
	for (phase, phase_results) in by_phase {
		let phase_passed = phase_results.iter().filter(|r| r.success).count();
		let phase_total = phase_results.len();
		info!("  {}: {}/{} passed", phase, phase_passed, phase_total);
	}

	info!("Test data and logs available in: test_data/");
}
