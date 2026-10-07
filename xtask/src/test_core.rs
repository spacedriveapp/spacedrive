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
	/// The crate the suite lives in: `sd-core` unless a suite says otherwise.
	pub package: &'static str,
	/// Specific args that go between the common prefix and suffix
	pub test_args: &'static [&'static str],
	/// Cargo features the suite's build turns on, or `None` for the default
	/// set.
	pub features: Option<&'static str>,
	/// Which CI job runs it.
	pub group: Group,
}

/// The CI job a suite belongs to. Each group is one runner, so the split is
/// about wall time: the integration group sits near the hour and anything
/// new goes in its own job rather than on top of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
	/// The `--lib` suite
	Unit,
	/// The original `--test` suites
	Integration,
	/// The acceptance-matrix suites (docs/core/acceptance/)
	Acceptance,
}

impl TestSuite {
	/// Build complete cargo test command arguments
	pub fn build_args(&self) -> Vec<&str> {
		let mut args = vec!["test", "-p", self.package];
		if let Some(features) = self.features {
			args.extend_from_slice(&["--features", features]);
		}
		args.extend_from_slice(self.test_args);
		args.extend_from_slice(&["--", "--test-threads=1", "--nocapture"]);
		args
	}
}

/// A suite in the `sd-core` crate, which is where most of them live.
const fn core(name: &'static str, test_args: &'static [&'static str]) -> TestSuite {
	TestSuite {
		name,
		package: "sd-core",
		test_args,
		features: None,
		group: Group::Integration,
	}
}

/// A suite in the acceptance group.
const fn acceptance(
	package: &'static str,
	name: &'static str,
	test_args: &'static [&'static str],
) -> TestSuite {
	TestSuite {
		name,
		package,
		test_args,
		features: None,
		group: Group::Acceptance,
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
		package: "sd-core",
		test_args: &["--lib"],
		features: None,
		group: Group::Unit,
	},
	core(
		"Database migration test",
		&["--test", "database_migration_test"],
	),
	core("Library test", &["--test", "library_test"]),
	core("Indexing rules test", &["--test", "indexing_rules_test"]),
	core("Watcher test", &["--test", "watcher_test"]),
	core("File move test", &["--test", "file_move_test"]),
	core(
		"Volume detection test",
		&["--test", "volume_detection_test"],
	),
	core("Volume tracking test", &["--test", "volume_tracking_test"]),
	core(
		"Typescript bridge test",
		&["--test", "typescript_bridge_test"],
	),
	core(
		"Typescript search bridge test",
		&["--test", "typescript_search_bridge_test"],
	),
	core(
		"Normalized cache fixtures test",
		&["--test", "normalized_cache_fixtures_test"],
	),
	core("Device pairing test", &["--test", "device_pairing_test"]),
	core("File copy pull test", &["--test", "file_copy_pull_test"]),
	core("File transfer test", &["--test", "file_transfer_test"]),
	core(
		"File transfer with restart test",
		&["--test", "file_transfer_with_restart_test"],
	),
	core(
		"Cross device copy test",
		&["--test", "cross_device_copy_test"],
	),
	core("Sync setup test", &["--test", "sync_setup_test"]),
	core("Sync backfill test", &["--test", "sync_backfill_test"]),
	core("Sync catch-up test", &["--test", "sync_catchup_test"]),
	core("Library join test", &["--test", "library_join_test"]),
	core("Dedupe own hash test", &["--test", "dedupe_own_hash_test"]),
	// R8 source runtime acceptance (docs/core/acceptance/source-runtime.md):
	// the single-daemon rows, the two-process replication rows, and the
	// store crate's own suites, which carry the store-level rows.
	acceptance(
		"sd-core",
		"Source runtime acceptance test",
		&["--test", "source_runtime_acceptance_test"],
	),
	acceptance(
		"sd-core",
		"Source replication test",
		&["--test", "source_replication_test"],
	),
	acceptance("sd-store", "Store crate tests", &[]),
	// FDA entries drop and file operations acceptance
	// (docs/core/acceptance/entries-drop-and-file-operations.md): the
	// single-daemon rows, plus the product-behavior suites the matrix cites
	// that were not yet registered.
	acceptance(
		"sd-core",
		"Entries drop acceptance test",
		&["--test", "entries_drop_acceptance_test"],
	),
	acceptance(
		"sd-core",
		"Copy action test",
		&["--test", "copy_action_test"],
	),
	acceptance(
		"sd-core",
		"Delete strategy test",
		&["--test", "delete_strategy_test"],
	),
	acceptance("sd-core", "Search test", &["--test", "search_test"]),
	acceptance(
		"sd-core",
		"Folder rename test",
		&["--test", "folder_rename_test"],
	),
	acceptance(
		"sd-core",
		"Resource events test",
		&["--test", "resource_events_test"],
	),
	// Locked volumes L1 and L2 (docs/core/acceptance/volumes.md): loop-mounted
	// volumes unmounted with their mount points left behind. Skips with a
	// reason where there is no sudo or no loop device.
	acceptance(
		"sd-core",
		"Locked volumes acceptance test",
		&["--test", "locked_volumes_acceptance_test"],
	),
	// The extension runtime. Its own build with the `wasm` feature: wasmer
	// in sd-core adds minutes to every test binary link, which took the
	// integration group from 51 to 78 minutes when the whole group carried
	// the feature.
	TestSuite {
		name: "WASM extension test",
		package: "sd-core",
		test_args: &["--test", "wasm_extension_test"],
		features: Some("wasm"),
		group: Group::Acceptance,
	},
	// core("Sync event log test", &["--test", "sync_event_log_test"]),
	// core("Sync metrics test", &["--test", "sync_metrics_test"]),
	// core("Sync backfill test", &["--test", "sync_backfill_test"]),
];

/// Which part of `CORE_TESTS` to run
///
/// CI runs each group as its own job so they compile and run in parallel;
/// locally the default runs everything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Selection {
	All,
	Group(Group),
}

impl Selection {
	fn includes(self, suite: &TestSuite) -> bool {
		match self {
			Selection::All => true,
			Selection::Group(group) => suite.group == group,
		}
	}
}

/// Test result for a single test suite
#[derive(Debug)]
pub struct TestResult {
	pub name: String,
	pub passed: bool,
}

/// Run the selected core test suites with progress tracking
pub fn run_tests(verbose: bool, selection: Selection) -> Result<Vec<TestResult>> {
	let suites: Vec<&TestSuite> = CORE_TESTS
		.iter()
		.filter(|suite| selection.includes(suite))
		.collect();
	let total_tests = suites.len();
	if total_tests == 0 {
		anyhow::bail!("{selection:?} selects no suite in CORE_TESTS; refusing to report a pass");
	}
	let mut results = Vec::new();

	println!();
	println!("{}", "Spacedrive Core Tests Runner".bright_cyan().bold());
	println!("Running {} test suite(s)\n", total_tests);

	let overall_start = Instant::now();

	for (index, test_suite) in suites.into_iter().enumerate() {
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
