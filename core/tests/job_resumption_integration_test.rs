//! Integration test for job resumption at various interruption points
//!
//! This test generates benchmark data and tests job resumption by interrupting
//! indexing jobs at different phases and progress points, then verifying they
//! can resume and complete successfully.

use sd_core::{
    infra::action::LibraryAction,
    ops::{
        indexing::IndexMode,
        locations::add::{action::LocationAddAction, action::LocationAddInput},
    },
};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
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

/// Test data directory in the repo for inspection
const TEST_DATA_DIR: &str = "data";

/// Benchmark recipe name to use for test data generation
const TEST_RECIPE_NAME: &str = "shape_medium";

/// Path where the benchmark data will be generated (relative to workspace root)
const TEST_INDEXING_DATA_PATH: &str = "benchdata/shape_medium";

/// Different interruption points to test
#[derive(Debug, Clone)]
enum InterruptionPoint {
    /// Interrupt during discovery phase at X% progress
    Discovery(u8),
    /// Interrupt during processing phase at X% progress
    Processing(u8),
    /// Interrupt during content identification at X% progress
    ContentIdentification(u8),
    /// Interrupt during aggregation phase
    Aggregation,
}

/// Test result for a single interruption scenario
#[derive(Debug)]
struct TestResult {
    interruption_point: InterruptionPoint,
    success: bool,
    error: Option<String>,
    job_log_path: Option<PathBuf>,
    daemon_log_path: Option<PathBuf>,
}

/// Main integration test
#[tokio::test]
async fn test_job_resumption_at_various_points() {
    // Initialize tracing for test debugging
    let _ = tracing_subscriber::fmt()
        .with_env_filter("warn,sd_core=info")
        .try_init();

    info!("Starting job resumption integration test");

    // Generate benchmark data (or use existing data)
    info!("Preparing test data");
    let indexing_data_path = generate_test_data().await.expect("Failed to prepare test data");

    // Define interruption points to test
    // For quick testing, comment out all but one interruption point
    let interruption_points = vec![
        InterruptionPoint::ContentIdentification(30),
        // InterruptionPoint::Discovery(25),
        // InterruptionPoint::Discovery(75),
        // InterruptionPoint::Processing(10),
        // InterruptionPoint::Processing(50),
        // InterruptionPoint::Processing(90),
        // InterruptionPoint::ContentIdentification(80),
        // InterruptionPoint::Aggregation,
    ];

    let mut results = Vec::new();
    let total_tests = interruption_points.len();

    // Test each interruption point
    for (i, interruption_point) in interruption_points.into_iter().enumerate() {
        info!("Testing interruption point {:?} ({}/{})", interruption_point, i + 1, total_tests);

        let result = test_single_interruption_point(
            &indexing_data_path,
            interruption_point.clone(),
            i,
        ).await;

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

    info!("All job resumption tests passed! 🎉");
}

/// Generate test data using benchmark data generation
async fn generate_test_data() -> Result<PathBuf, Box<dyn std::error::Error>> {
    use std::process::Command;

    let current_dir = std::env::current_dir()?;
    info!("Current directory: {}", current_dir.display());

    // Use relative path from workspace root (tests run from core/ directory)
    let indexing_data_path = if current_dir.ends_with("core") {
        current_dir.parent().unwrap().join(TEST_INDEXING_DATA_PATH)
    } else {
        current_dir.join(TEST_INDEXING_DATA_PATH)
    };

    // Check if data already exists
    if indexing_data_path.exists() && indexing_data_path.is_dir() {
        // Check if directory has files
        let entries: Vec<_> = std::fs::read_dir(&indexing_data_path)?
            .collect::<Result<Vec<_>, _>>()?;

        if !entries.is_empty() {
            info!("Test data already exists at: {}, skipping generation", indexing_data_path.display());
            return Ok(indexing_data_path);
        }
    }

    // Run benchmark data generation using existing recipe
    info!("Generating test data using recipe: {}", TEST_RECIPE_NAME);
    let recipe_path = current_dir.join("benchmarks/recipes").join(format!("{}.yaml", TEST_RECIPE_NAME));
    info!("Recipe path: {}", recipe_path.display());

    let output = Command::new("cargo")
        .args([
            "run", "-p", "sd-bench", "--bin", "sd-bench", "--",
            "mkdata",
            "--recipe", recipe_path.to_str().unwrap(),
        ])
        .current_dir(&current_dir)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(format!(
            "Benchmark data generation failed:\nSTDOUT: {}\nSTDERR: {}",
            stdout, stderr
        ).into());
    }

    info!("Generated test data at: {}", indexing_data_path.display());
    Ok(indexing_data_path)
}

/// Test a single interruption point scenario
async fn test_single_interruption_point(
    indexing_data_path: &PathBuf,
    interruption_point: InterruptionPoint,
    test_index: usize,
) -> TestResult {
    let test_name = format!("test_{:02}_{:?}", test_index, interruption_point);
    let test_data_path = PathBuf::from(TEST_DATA_DIR);
    let core_data_path = test_data_path.join(&test_name);

    // Clean core data directory for this test
    if core_data_path.exists() {
        let _ = std::fs::remove_dir_all(&core_data_path);
    }
    std::fs::create_dir_all(&core_data_path).expect("Failed to create core data directory");

    info!("Testing {} with data at {}", test_name, indexing_data_path.display());

    // Phase 1: Start indexing and interrupt at specified point
    let interrupt_result = start_and_interrupt_job(
        &core_data_path,
        indexing_data_path,
        &interruption_point,
    ).await;

    let job_id = match interrupt_result {
        Ok(job_id) => job_id,
        Err(error) => {
            return TestResult {
                interruption_point,
                success: false,
                error: Some(format!("Failed to interrupt job: {}", error)),
                job_log_path: None,
                daemon_log_path: None,
            };
        }
    };

    // Brief pause to ensure clean shutdown
    sleep(Duration::from_secs(1)).await;

    // Phase 2: Resume and complete the job
    let resume_result = resume_and_complete_job(
        &core_data_path,
        indexing_data_path,
        job_id,
    ).await;

    match resume_result {
        Ok((job_log_path, daemon_log_path)) => TestResult {
            interruption_point,
            success: true,
            error: None,
            job_log_path: Some(job_log_path),
            daemon_log_path: Some(daemon_log_path),
        },
        Err(error) => TestResult {
            interruption_point,
            success: false,
            error: Some(format!("Failed to resume job: {}", error)),
            job_log_path: None,
            daemon_log_path: None,
        },
    }
}

/// Start indexing job and interrupt at specified point
async fn start_and_interrupt_job(
    core_data_path: &PathBuf,
    indexing_data_path: &PathBuf,
    interruption_point: &InterruptionPoint,
) -> Result<Uuid, Box<dyn std::error::Error>> {
    info!("Starting job and waiting for interruption point: {:?}", interruption_point);

    // Create core with isolated data directory
    let core = sd_core::Core::new_with_config(core_data_path.clone()).await?;
    let core_context = core.context.clone();

    // Create library
    let library = core_context.libraries().await
        .create_library("Test Library".to_string(), None, core_context.clone())
        .await?;

    // Create location add action to automatically trigger indexing
    let location_input = LocationAddInput {
        path: indexing_data_path.clone(),
        name: Some("Test Location".to_string()),
        mode: IndexMode::Content,
    };

    let location_action = LocationAddAction::from_input(location_input)
        .map_err(|e| format!("Failed to create location action: {}", e))?;

    // Dispatch the location add action through the action manager
    let action_manager = core_context.action_manager.read().await;
    let action_manager = action_manager.as_ref()
        .ok_or("Action manager not initialized")?;

    let location_output = action_manager
        .dispatch_library(Some(library.id()), location_action)
        .await
        .map_err(|e| format!("Failed to dispatch location add action: {}", e))?;

    // The location add action automatically creates an indexing job
    let job_id = location_output.job_id
        .ok_or("Location add action did not return a job ID")?;

    // Set up event monitoring
    let (interrupt_tx, mut interrupt_rx) = mpsc::channel(1);
    let should_interrupt = Arc::new(AtomicBool::new(false));
    let should_interrupt_clone = should_interrupt.clone();

    // Monitor events for interruption point
    let mut event_rx = core_context.events.subscribe();
    let interruption_point_clone = interruption_point.clone();
    tokio::spawn(async move {
        while let Ok(event) = event_rx.recv().await {
            if let sd_core::infra::event::Event::JobProgress { job_id: event_job_id, progress, message, .. } = event {
                if event_job_id == job_id.to_string() {
                    let message_str = message.as_deref().unwrap_or("");
                    info!("Job progress: {}% - {}", progress * 100.0, message_str);
                    let should_interrupt_now = match &interruption_point_clone {
                        InterruptionPoint::Discovery(target_percent) => {
                            // Interrupt during discovery phase if we're at or past target percentage
                            message_str.contains("Discovery") && progress >= (*target_percent as f64 * 0.01)
                        },
                        InterruptionPoint::Processing(target_percent) => {
                            // Interrupt during processing phase if we're at or past target percentage
                            message_str.contains("Processing") && progress >= (*target_percent as f64 * 0.01)
                        },
                        InterruptionPoint::ContentIdentification(target_percent) => {
                            // Interrupt during content identification if we're at or past target percentage
                            (message_str.contains("Content") || message_str.contains("content identities")) &&
                            progress >= (*target_percent as f64 * 0.01)
                        },
                        InterruptionPoint::Aggregation => {
                            // Interrupt as soon as we enter aggregation phase
                            message_str.contains("Aggregation")
                        },
                    };

                    if should_interrupt_now && !should_interrupt_clone.load(Ordering::Relaxed) {
                        info!("Triggering interrupt at {}% in phase: {}",
                            progress, message_str);
                        should_interrupt_clone.store(true, Ordering::Relaxed);
                        let _ = interrupt_tx.send(()).await;
                    }
                }
            }
        }
    });

    // Wait for interruption point or timeout
    let interrupt_timeout = timeout(Duration::from_secs(120), interrupt_rx.recv()).await;

    match interrupt_timeout {
        Ok(Some(())) => {
            info!("Interruption point reached, shutting down core");
            // Shutdown core gracefully
            core.shutdown().await?;
            Ok(job_id)
        },
        Ok(None) => Err("Interrupt channel closed unexpectedly".into()),
        Err(_) => Err("Timeout waiting for interruption point".into()),
    }
}

/// Resume and complete the interrupted job
async fn resume_and_complete_job(
    core_data_path: &PathBuf,
    _indexing_data_path: &PathBuf,
    job_id: Uuid,
) -> Result<(PathBuf, PathBuf), Box<dyn std::error::Error>> {
    info!("Resuming job {} and waiting for completion", job_id);

    // Create core again (simulating daemon restart)
    let core = sd_core::Core::new_with_config(core_data_path.clone()).await?;
    let core_context = core.context.clone();

    // Get the library (should auto-load)
    let libraries = core_context.libraries().await.list().await;
    let _library = libraries.first()
        .ok_or("No library found after restart")?;

    // Set up completion monitoring
    let (completion_tx, mut completion_rx) = mpsc::channel(1);
    let job_completed = Arc::new(AtomicBool::new(false));
    let job_completed_clone = job_completed.clone();

    // Monitor for job completion
    let mut event_rx = core_context.events.subscribe();
    tokio::spawn(async move {
        while let Ok(event) = event_rx.recv().await {
            match event {
                sd_core::infra::event::Event::JobCompleted { job_id: event_job_id, .. } => {
                    if event_job_id == job_id.to_string() {
                        info!("Job {} completed successfully", job_id);
                        job_completed_clone.store(true, Ordering::Relaxed);
                        let _ = completion_tx.send(Ok(())).await;
                        break;
                    }
                },
                sd_core::infra::event::Event::JobFailed { job_id: event_job_id, error, .. } => {
                    if event_job_id == job_id.to_string() {
                        warn!("Job {} failed: {}", job_id, error);
                        let _ = completion_tx.send(Err(error)).await;
                        break;
                    }
                },
                _ => {}
            }
        }
    });

    // Wait for completion or timeout
    let completion_timeout = timeout(Duration::from_secs(300), completion_rx.recv()).await;

    match completion_timeout {
        Ok(Some(Ok(()))) => {
            info!("Job completed successfully");

            // Collect log paths for inspection
            let job_log_path = core_data_path.join("job_logs").join(format!("{}.log", job_id));
            let daemon_log_path = core_data_path.join("daemon.log");

            // Shutdown core
            core.shutdown().await?;

            Ok((job_log_path, daemon_log_path))
        },
        Ok(Some(Err(error))) => {
            core.shutdown().await?;
            Err(format!("Job failed during resumption: {}", error).into())
        },
        Ok(None) => {
            core.shutdown().await?;
            Err("Completion channel closed unexpectedly".into())
        },
        Err(_) => {
            core.shutdown().await?;
            Err("Timeout waiting for job completion".into())
        },
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
            warn!("  {:?}: {}", result.interruption_point,
                result.error.as_ref().unwrap_or(&"Unknown error".to_string()));

            if let Some(job_log) = &result.job_log_path {
                warn!("    Job log: {}", job_log.display());
            }
            if let Some(daemon_log) = &result.daemon_log_path {
                warn!("    Daemon log: {}", daemon_log.display());
            }
        }
    }

    // Group results by interruption type
    let mut by_phase = std::collections::HashMap::new();
    for result in results {
        let phase = match &result.interruption_point {
            InterruptionPoint::Discovery(_) => "Discovery",
            InterruptionPoint::Processing(_) => "Processing",
            InterruptionPoint::ContentIdentification(_) => "Content Identification",
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

    info!("Test data and logs available in: {}", TEST_DATA_DIR);
}

