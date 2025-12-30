//! Core integration tests runner
//!
//! Single source of truth for all sd-core integration tests. This module defines
//! which tests should run when testing the core, used both by CI and local development.

use anyhow::{Context, Result};
use std::process::Command;
use std::time::Instant;

/// Test suite definition with name and cargo test arguments
#[derive(Debug, Clone)]
pub struct TestSuite {
	pub name: &'static str,
	pub args: &'static [&'static str],
}

/// All core integration tests that should run in CI and locally
///
/// This is the single source of truth for which tests to run.
/// Add or remove tests here and they'll automatically apply to both
/// CI workflows and local test scripts.
pub const CORE_TESTS: &[TestSuite] = &[
	TestSuite {
		name: "Library tests",
		args: &[
			"test",
			"-p",
			"sd-core",
			"--lib",
			"--",
			"--test-threads=1",
			"--nocapture",
		],
	},
	TestSuite {
		name: "Indexing test",
		args: &[
			"test",
			"-p",
			"sd-core",
			"--test",
			"indexing_test",
			"--",
			"--test-threads=1",
			"--nocapture",
		],
	},
	TestSuite {
		name: "Indexing rules test",
		args: &[
			"test",
			"-p",
			"sd-core",
			"--test",
			"indexing_rules_test",
			"--",
			"--test-threads=1",
			"--nocapture",
		],
	},
	// TestSuite {
	// 	name: "Indexing responder reindex test",
	// 	args: &[
	// 		"test",
	// 		"-p",
	// 		"sd-core",
	// 		"--test",
	// 		"indexing_responder_reindex_test",
	// 		"--",
	// 		"--test-threads=1",
	// 		"--nocapture",
	// 	],
	// },
	TestSuite {
		name: "Sync backfill test",
		args: &[
			"test",
			"-p",
			"sd-core",
			"--test",
			"sync_backfill_test",
			"--",
			"--test-threads=1",
			"--nocapture",
		],
	},
	TestSuite {
		name: "Sync backfill race test",
		args: &[
			"test",
			"-p",
			"sd-core",
			"--test",
			"sync_backfill_race_test",
			"--",
			"--test-threads=1",
			"--nocapture",
		],
	},
	// TestSuite {
	// 	name: "Sync event log test",
	// 	args: &[
	// 		"test",
	// 		"-p",
	// 		"sd-core",
	// 		"--test",
	// 		"sync_event_log_test",
	// 		"--",
	// 		"--test-threads=1",
	// 		"--nocapture",
	// 	],
	// },
	// TestSuite {
	// 	name: "Sync metrics test",
	// 	args: &[
	// 		"test",
	// 		"-p",
	// 		"sd-core",
	// 		"--test",
	// 		"sync_metrics_test",
	// 		"--",
	// 		"--test-threads=1",
	// 		"--nocapture",
	// 	],
	// },
	// TestSuite {
	// 	name: "Sync realtime test",
	// 	args: &[
	// 		"test",
	// 		"-p",
	// 		"sd-core",
	// 		"--test",
	// 		"sync_realtime_test",
	// 		"--",
	// 		"--test-threads=1",
	// 		"--nocapture",
	// 	],
	// },
	TestSuite {
		name: "Sync setup test",
		args: &[
			"test",
			"-p",
			"sd-core",
			"--test",
			"sync_setup_test",
			"--",
			"--test-threads=1",
			"--nocapture",
		],
	},
	TestSuite {
		name: "File sync simple test",
		args: &[
			"test",
			"-p",
			"sd-core",
			"--test",
			"file_sync_simple_test",
			"--",
			"--test-threads=1",
			"--nocapture",
		],
	},
	// TestSuite {
	// 	name: "File sync test",
	// 	args: &[
	// 		"test",
	// 		"-p",
	// 		"sd-core",
	// 		"--test",
	// 		"file_sync_test",
	// 		"--",
	// 		"--test-threads=1",
	// 		"--nocapture",
	// 	],
	// },
	TestSuite {
		name: "Database migration test",
		args: &[
			"test",
			"-p",
			"sd-core",
			"--test",
			"database_migration_test",
			"--",
			"--test-threads=1",
			"--nocapture",
		],
	},
];

/// Test result for a single test suite
#[derive(Debug)]
pub struct TestResult {
	pub name: String,
	pub passed: bool,
}

/// Run all core integration tests with progress tracking
pub fn run_tests(verbose: bool) -> Result<Vec<TestResult>> {
	let total_tests = CORE_TESTS.len();
	let mut results = Vec::new();

	println!("════════════════════════════════════════════════════════════════");
	println!("  Spacedrive Core Tests Runner");
	println!("  Running {} test suite(s)", total_tests);
	println!("════════════════════════════════════════════════════════════════");
	println!();

	let overall_start = Instant::now();

	for (index, test_suite) in CORE_TESTS.iter().enumerate() {
		let current = index + 1;

		println!("[{}/{}] Running: {}", current, total_tests, test_suite.name);
		println!("────────────────────────────────────────────────────────────────");

		let test_start = Instant::now();

		let mut cmd = Command::new("cargo");
		cmd.args(test_suite.args);

		if !verbose {
			cmd.stdout(std::process::Stdio::null());
			cmd.stderr(std::process::Stdio::null());
		}

		let status = cmd
			.status()
			.context(format!("Failed to execute test: {}", test_suite.name))?;

		let duration = test_start.elapsed().as_secs();
		let exit_code = status.code().unwrap_or(-1);
		let passed = status.success();

		if passed {
			println!("✓ PASSED ({}s)", duration);
		} else {
			println!("✗ FAILED (exit code: {}, {}s)", exit_code, duration);
		}
		println!();

		results.push(TestResult {
			name: test_suite.name.to_string(),
			passed,
		});
	}

	let total_duration = overall_start.elapsed();
	print_summary(&results, total_duration);

	Ok(results)
}

/// Print test results summary
fn print_summary(results: &[TestResult], total_duration: std::time::Duration) {
	let total_tests = results.len();
	let passed_tests: Vec<_> = results.iter().filter(|r| r.passed).collect();
	let failed_tests: Vec<_> = results.iter().filter(|r| !r.passed).collect();

	let minutes = total_duration.as_secs() / 60;
	let seconds = total_duration.as_secs() % 60;

	println!("════════════════════════════════════════════════════════════════");
	println!("  Test Results Summary");
	println!("════════════════════════════════════════════════════════════════");
	println!();
	println!("Total time: {}m {}s", minutes, seconds);
	println!();

	if !passed_tests.is_empty() {
		println!("✓ Passed ({}/{}):", passed_tests.len(), total_tests);
		for result in passed_tests {
			println!("  ✓ {}", result.name);
		}
		println!();
	}

	if !failed_tests.is_empty() {
		println!("✗ Failed ({}/{}):", failed_tests.len(), total_tests);
		for result in failed_tests {
			println!("  ✗ {}", result.name);
		}
		println!();
	}

	println!("════════════════════════════════════════════════════════════════");
	println!();
}
