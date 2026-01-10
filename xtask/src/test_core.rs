//! Core integration tests runner
//!
//! Single source of truth for all sd-core integration tests. This module defines
//! which tests should run when testing the core, used both by CI and local development.

use anyhow::{Context, Result};
use owo_colors::OwoColorize;
use std::process::Command;
use std::time::Instant;

/// Test suite definition with name and specific test arguments
#[derive(Debug, Clone)]
pub struct TestSuite {
	pub name: &'static str,
	/// Specific args that go between the common prefix and suffix
	pub test_args: &'static [&'static str],
}

impl TestSuite {
	/// Build complete cargo test command arguments
	pub fn build_args(&self) -> Vec<&str> {
		let mut args = vec!["test", "-p", "sd-core"];
		args.extend_from_slice(self.test_args);
		args.extend_from_slice(&["--", "--test-threads=1", "--nocapture"]);
		args
	}
}

/// All core integration tests that should run in CI and locally
///
/// This is the single source of truth for which tests to run.
/// Add or remove tests here and they'll automatically apply to both
/// CI workflows and local test scripts.
pub const CORE_TESTS: &[TestSuite] = &[
	TestSuite {
		name: "All core unit tests",
		test_args: &["--lib"],
	},
	TestSuite {
		name: "Database migration test",
		test_args: &["--test", "database_migration_test"],
	},
	TestSuite {
		name: "Library test",
		test_args: &["--test", "library_test"],
	},
	TestSuite {
		name: "Indexing test",
		test_args: &["--test", "indexing_test"],
	},
	TestSuite {
		name: "Indexing rules test",
		test_args: &["--test", "indexing_rules_test"],
	},
	TestSuite {
		name: "Indexing responder reindex test",
		test_args: &["--test", "indexing_responder_reindex_test"],
	},
	TestSuite {
		name: "File structure test",
		test_args: &["--test", "file_structure_test"],
	},
	TestSuite {
		name: "FS watcher test",
		test_args: &["--test", "fs_watcher_test"],
	},
	TestSuite {
		name: "Ephemeral watcher test",
		test_args: &["--test", "ephemeral_watcher_test"],
	},
	TestSuite {
		name: "File move test",
		test_args: &["--test", "file_move_test"],
	},
	TestSuite {
		name: "Entry move integrity test",
		test_args: &["--test", "entry_move_integrity_test"],
	},
	TestSuite {
		name: "Volume detection test",
		test_args: &["--test", "volume_detection_test"],
	},
	TestSuite {
		name: "Volume tracking test",
		test_args: &["--test", "volume_tracking_test"],
	},
	TestSuite {
		name: "Typescript bridge test",
		test_args: &["--test", "typescript_bridge_test"],
	},
	// TestSuite {
	// 	name: "Typescript search bridge test",
	// 	test_args: &["--test", "typescript_search_bridge_test"],
	// },
	TestSuite {
		name: "Normalized cache fixtures test",
		test_args: &["--test", "normalized_cache_fixtures_test"],
	},
	TestSuite {
		name: "Device pairing test",
		test_args: &["--test", "device_pairing_test"],
	},
	TestSuite {
		name: "File copy pull test",
		test_args: &["--test", "file_copy_pull_test"],
	},
	TestSuite {
		name: "File transfer test",
		test_args: &["--test", "file_transfer_test"],
	},
	TestSuite {
		name: "Cross device copy test",
		test_args: &["--test", "cross_device_copy_test"],
	},
	TestSuite {
		name: "Sync setup test",
		test_args: &["--test", "sync_setup_test"],
	},
	// TestSuite {
	// 	name: "Sync event log test",
	// 	test_args: &["--test", "sync_event_log_test"],
	// },
	// TestSuite {
	// 	name: "Sync metrics test",
	// 	test_args: &["--test", "sync_metrics_test"],
	// },
	// TestSuite {
	// 	name: "Sync realtime test",
	// 	test_args: &["--test", "sync_realtime_test"],
	// },
	// TestSuite {
	// 	name: "File sync simple test",
	// 	test_args: &["--test", "file_sync_simple_test"],
	// },
	// TestSuite {
	// 	name: "File sync test",
	// 	test_args: &["--test", "file_sync_test"],
	// },

	// TestSuite {
	// 	name: "Sync backfill test",
	// 	test_args: &["--test", "sync_backfill_test"],
	// },
	// TestSuite {
	// 	name: "Sync backfill race test",
	// 	test_args: &["--test", "sync_backfill_race_test"],
	// },
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

	println!();
	println!("{}", "Spacedrive Core Tests Runner".bright_cyan().bold());
	println!("Running {} test suite(s)\n", total_tests);

	let overall_start = Instant::now();

	for (index, test_suite) in CORE_TESTS.iter().enumerate() {
		let current = index + 1;

		print!("[{}/{}] ", current, total_tests);
		print!("{} ", "●".bright_blue());
		println!("{}", test_suite.name.bold());

		let args_display = test_suite.test_args.join(" ");
		println!("      {} {}", "args:".dimmed(), args_display.dimmed());

		let test_start = Instant::now();

		let mut cmd = Command::new("cargo");
		cmd.args(test_suite.build_args());

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
			println!("      {} {}s\n", "✓".bright_green(), duration);
		} else {
			println!(
				"      {} {}s (exit code: {})\n",
				"✗".bright_red(),
				duration,
				exit_code
			);
		}

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

	println!("{}", "Test Results Summary".bright_cyan().bold());
	println!("{} {}m {}s\n", "Total time:".dimmed(), minutes, seconds);

	if !passed_tests.is_empty() {
		println!(
			"{} {}/{}",
			"✓ Passed".bright_green().bold(),
			passed_tests.len(),
			total_tests
		);
		for result in passed_tests {
			println!("  {} {}", "✓".bright_green(), result.name);
		}
		println!();
	}

	if !failed_tests.is_empty() {
		println!(
			"{} {}/{}",
			"✗ Failed".bright_red().bold(),
			failed_tests.len(),
			total_tests
		);
		for result in failed_tests {
			println!("  {} {}", "✗".bright_red(), result.name);
		}
		println!();
	}
}
