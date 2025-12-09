//! Ephemeral Watcher Integration Test
//!
//! Tests the real-time file system monitoring functionality for ephemeral indexing
//! through a comprehensive "story" of file operations, verifying that the watcher
//! correctly detects and updates the in-memory ephemeral index for all filesystem changes.

use sd_core::{
	context::CoreContext,
	library::Library,
	ops::indexing::{
		job::{IndexScope, IndexerJob, IndexerJobConfig},
		state::EntryKind,
	},
	service::{watcher::FsWatcherService, watcher::FsWatcherServiceConfig, Service},
	Core,
};
use sd_fs_watcher::FsEvent;
use std::{path::PathBuf, sync::Arc, time::Duration};
use tempfile::TempDir;
use tokio::sync::Mutex;
use tokio::time::timeout;

// ============================================================================
// Event Collector for Debugging
// ============================================================================

/// Collects FsEvents for diagnostic output
struct EventCollector {
	events: Arc<Mutex<Vec<(std::time::Instant, FsEvent)>>>,
	start_time: std::time::Instant,
}

impl EventCollector {
	fn new() -> Self {
		Self {
			events: Arc::new(Mutex::new(Vec::new())),
			start_time: std::time::Instant::now(),
		}
	}

	/// Start collecting events from a watcher
	fn start_collecting(&self, watcher: &FsWatcherService) {
		let events = self.events.clone();
		let mut rx = watcher.subscribe();

		tokio::spawn(async move {
			loop {
				match rx.recv().await {
					Ok(event) => {
						let mut events_lock = events.lock().await;
						events_lock.push((std::time::Instant::now(), event));
					}
					Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
					Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
						eprintln!("Event collector lagged by {} events", n);
					}
				}
			}
		});
	}

	/// Dump collected events to a file
	async fn dump_to_file(&self, path: &std::path::Path) -> std::io::Result<()> {
		use std::io::Write;

		let events = self.events.lock().await;
		let mut file = std::fs::File::create(path)?;

		writeln!(file, "=== FsWatcher Event Log ===")?;
		writeln!(file, "Total events collected: {}", events.len())?;
		writeln!(file, "")?;

		for (i, (timestamp, event)) in events.iter().enumerate() {
			let elapsed = timestamp.duration_since(self.start_time);
			writeln!(
				file,
				"[{:03}] +{:.3}s | {:?}",
				i,
				elapsed.as_secs_f64(),
				event.kind
			)?;
			writeln!(file, "         path: {}", event.path.display())?;
			if let Some(is_dir) = event.is_directory {
				writeln!(file, "         is_directory: {}", is_dir)?;
			}
			writeln!(file, "")?;
		}

		writeln!(file, "=== End of Event Log ===")?;

		Ok(())
	}

	/// Print summary to console
	async fn print_summary(&self) {
		let events = self.events.lock().await;

		println!("\n=== Event Summary ===");
		println!("Total events: {}", events.len());

		let mut creates = 0;
		let mut modifies = 0;
		let mut removes = 0;
		let mut renames = 0;

		for (_, event) in events.iter() {
			match &event.kind {
				sd_fs_watcher::FsEventKind::Create => creates += 1,
				sd_fs_watcher::FsEventKind::Modify => modifies += 1,
				sd_fs_watcher::FsEventKind::Remove => removes += 1,
				sd_fs_watcher::FsEventKind::Rename { .. } => renames += 1,
			}
		}

		println!("  Creates: {}", creates);
		println!("  Modifies: {}", modifies);
		println!("  Removes: {}", removes);
		println!("  Renames: {}", renames);
		println!("===================\n");
	}
}

// ============================================================================
// Test Harness
// ============================================================================

/// Test harness for ephemeral watcher testing with reusable operations
struct TestHarness {
	_core_data_dir: TempDir,
	core: Arc<Core>,
	library: Arc<Library>,
	test_dir: PathBuf,
	watcher: Arc<FsWatcherService>,
	context: Arc<CoreContext>,
	event_collector: EventCollector,
}

impl TestHarness {
	/// Setup the test environment with core and ephemeral watching
	async fn setup() -> Result<Self, Box<dyn std::error::Error>> {
		// Setup logging
		let _ = tracing_subscriber::fmt()
			.with_env_filter("sd_core=debug,ephemeral_watcher_test=debug")
			.try_init();

		// Create core
		let temp_dir = TempDir::new()?;
		let core = Core::new(temp_dir.path().to_path_buf()).await?;

		println!("✓ Created core");

		// Create library
		let library = core
			.libraries
			.create_library("Ephemeral Test", None, core.context.clone())
			.await?;

		println!("✓ Created library: {}", library.id());

		// Create test directory in user's home (cross-platform)
		let home_dir = if cfg!(windows) {
			std::env::var("USERPROFILE").unwrap_or_else(|_| {
				let drive = std::env::var("HOMEDRIVE").unwrap_or_else(|_| "C:".to_string());
				let path = std::env::var("HOMEPATH").unwrap_or_else(|_| {
					let username =
						std::env::var("USERNAME").expect("Could not determine username on Windows");
					format!("\\Users\\{}", username)
				});
				format!("{}{}", drive, path)
			})
		} else {
			std::env::var("HOME").expect("HOME environment variable not set")
		};

		let test_dir = PathBuf::from(home_dir).join("SD_EPHEMERAL_TEST_DIR");

		// Clear and recreate test directory
		if test_dir.exists() {
			tokio::fs::remove_dir_all(&test_dir).await?;
		}
		tokio::fs::create_dir_all(&test_dir).await?;
		println!("✓ Created test directory: {}", test_dir.display());

		// Create initial file
		tokio::fs::write(test_dir.join("initial.txt"), "initial content").await?;

		// Start the watcher service
		let watcher_config = FsWatcherServiceConfig::default();
		let watcher = Arc::new(FsWatcherService::new(core.context.clone(), watcher_config));
		watcher.init_handlers().await;
		watcher.start().await?;
		println!("✓ Started watcher service");

		// Create event collector and start collecting
		let event_collector = EventCollector::new();
		event_collector.start_collecting(&watcher);
		println!("✓ Started event collector");

		// Run ephemeral indexing job
		let sd_path = sd_core::domain::addressing::SdPath::local(test_dir.clone());
		let config = IndexerJobConfig::ephemeral_browse(sd_path, IndexScope::Current);
		let mut indexer_job = IndexerJob::new(config);

		// Get the global ephemeral index to share with the job
		let ephemeral_index = core.context.ephemeral_cache().get_global_index();
		indexer_job.set_ephemeral_index(ephemeral_index);

		// Dispatch job
		let job_handle = library.jobs().dispatch(indexer_job).await?;

		// Wait for indexing to complete
		timeout(Duration::from_secs(60), job_handle.wait()).await??;
		println!("✓ Ephemeral index completed");

		// Mark indexing complete and register for watching
		core.context
			.ephemeral_cache()
			.mark_indexing_complete(&test_dir);

		// Add ephemeral watch
		watcher.watch_ephemeral(test_dir.clone()).await?;
		println!("✓ Added ephemeral watch for: {}", test_dir.display());

		// Give the watcher a moment to settle
		tokio::time::sleep(Duration::from_millis(500)).await;

		let context = core.context.clone();

		Ok(Self {
			_core_data_dir: temp_dir,
			core: Arc::new(core),
			library,
			test_dir,
			watcher,
			context,
			event_collector,
		})
	}

	/// Get the full path for a relative file/directory name
	fn path(&self, name: &str) -> PathBuf {
		self.test_dir.join(name)
	}

	/// Create a file with content
	async fn create_file(
		&self,
		name: &str,
		content: &str,
	) -> Result<(), Box<dyn std::error::Error>> {
		let path = self.path(name);
		tokio::fs::write(&path, content).await?;
		println!("Created file: {}", name);
		Ok(())
	}

	/// Modify a file's content
	async fn modify_file(
		&self,
		name: &str,
		new_content: &str,
	) -> Result<(), Box<dyn std::error::Error>> {
		let path = self.path(name);
		tokio::fs::write(&path, new_content).await?;
		println!("Modified file: {}", name);
		Ok(())
	}

	/// Delete a file
	async fn delete_file(&self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
		let path = self.path(name);
		tokio::fs::remove_file(&path).await?;
		println!("Deleted file: {}", name);
		Ok(())
	}

	/// Rename/move a file
	async fn rename_file(&self, from: &str, to: &str) -> Result<(), Box<dyn std::error::Error>> {
		let from_path = self.path(from);
		let to_path = self.path(to);
		tokio::fs::rename(&from_path, &to_path).await?;
		println!("Renamed: {} -> {}", from, to);
		Ok(())
	}

	/// Create a directory
	async fn create_dir(&self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
		let path = self.path(name);
		tokio::fs::create_dir_all(&path).await?;
		println!("Created directory: {}", name);
		Ok(())
	}

	/// Verify entry exists in ephemeral index
	async fn verify_entry_exists(&self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
		let path = self.path(name);

		// Poll for the entry to appear (with timeout)
		let start = std::time::Instant::now();
		let timeout_duration = Duration::from_secs(10);

		while start.elapsed() < timeout_duration {
			let index = self.context.ephemeral_cache().get_global_index();
			let mut index_lock = index.write().await;
			if index_lock.get_entry(&path).is_some() {
				println!("✓ Entry exists in ephemeral index: {}", name);
				return Ok(());
			}
			drop(index_lock);
			tokio::time::sleep(Duration::from_millis(50)).await;
		}

		Err(format!(
			"Entry '{}' not found in ephemeral index after timeout",
			name
		)
		.into())
	}

	/// Verify entry does NOT exist in ephemeral index
	async fn verify_entry_not_exists(&self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
		let path = self.path(name);

		// Poll for the entry to be removed (with timeout)
		let start = std::time::Instant::now();
		let timeout_duration = Duration::from_secs(5);

		while start.elapsed() < timeout_duration {
			let index = self.context.ephemeral_cache().get_global_index();
			let mut index_lock = index.write().await;
			if index_lock.get_entry(&path).is_none() {
				println!("✓ Entry does not exist in ephemeral index: {}", name);
				return Ok(());
			}
			drop(index_lock);
			tokio::time::sleep(Duration::from_millis(100)).await;
		}

		Err(format!(
			"Entry '{}' should not exist but was found in ephemeral index after timeout",
			name
		)
		.into())
	}

	/// Verify entry is a directory
	async fn verify_is_directory(&self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
		let path = self.path(name);
		let index = self.context.ephemeral_cache().get_global_index();
		let mut index_lock = index.write().await;

		if let Some(entry) = index_lock.get_entry(&path) {
			if entry.kind == EntryKind::Directory {
				println!("✓ Entry '{}' is correctly marked as directory", name);
				return Ok(());
			} else {
				return Err(format!(
					"Entry '{}' should be a directory but kind={:?}",
					name, entry.kind
				)
				.into());
			}
		}
		Err(format!("Entry '{}' not found in index", name).into())
	}

	/// Verify entry is a file (not directory)
	async fn verify_is_file(&self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
		let path = self.path(name);
		let index = self.context.ephemeral_cache().get_global_index();
		let mut index_lock = index.write().await;

		if let Some(entry) = index_lock.get_entry(&path) {
			if entry.kind == EntryKind::File {
				println!("✓ Entry '{}' is correctly marked as file", name);
				return Ok(());
			} else {
				return Err(format!(
					"Entry '{}' should be a file but kind={:?}",
					name, entry.kind
				)
				.into());
			}
		}
		Err(format!("Entry '{}' not found in index", name).into())
	}

	/// Get current entry count in index for this test directory
	async fn get_entry_count(&self) -> usize {
		let index = self.context.ephemeral_cache().get_global_index();
		let index_lock = index.read().await;
		index_lock
			.entries()
			.iter()
			.filter(|(path, _)| path.starts_with(&self.test_dir))
			.count()
	}

	/// Verify expected entry count
	async fn verify_entry_count(&self, expected: usize) -> Result<(), Box<dyn std::error::Error>> {
		let count = self.get_entry_count().await;
		if count == expected {
			println!("✓ Entry count matches: {}", expected);
			Ok(())
		} else {
			// List actual entries for debugging
			let index = self.context.ephemeral_cache().get_global_index();
			let index_lock = index.read().await;
			let entries: Vec<_> = index_lock
				.entries()
				.iter()
				.filter(|(path, _)| path.starts_with(&self.test_dir))
				.map(|(path, entry)| {
					let kind_str = match entry.kind {
						EntryKind::Directory => "DIR",
						EntryKind::File => "FILE",
						EntryKind::Symlink => "LINK",
					};
					format!(
						"  - {} ({})",
						path.strip_prefix(&self.test_dir).unwrap_or(path).display(),
						kind_str
					)
				})
				.collect();

			Err(format!(
				"Entry count mismatch: expected {}, got {}\nActual entries:\n{}",
				expected,
				count,
				entries.join("\n")
			)
			.into())
		}
	}

	/// Print current index state (for debugging)
	async fn dump_index_state(&self) {
		let index = self.context.ephemeral_cache().get_global_index();
		let index_lock = index.read().await;

		println!("\n=== Ephemeral Index State ===");
		let mut count = 0;
		for (path, entry) in index_lock.entries().iter() {
			if path.starts_with(&self.test_dir) {
				let rel_path = path.strip_prefix(&self.test_dir).unwrap_or(path);
				let type_str = match entry.kind {
					EntryKind::Directory => "DIR ",
					EntryKind::File => "FILE",
					EntryKind::Symlink => "LINK",
				};
				println!("  {} {}", type_str, rel_path.display());
				count += 1;
			}
		}
		println!("Total entries: {}", count);
		println!("=============================\n");
	}

	/// Dump collected events to file and print summary
	async fn dump_events(&self) {
		// Print summary to console
		self.event_collector.print_summary().await;

		// Write detailed log to file
		let log_path = std::env::temp_dir().join("ephemeral_watcher_events.log");
		if let Err(e) = self.event_collector.dump_to_file(&log_path).await {
			eprintln!("Failed to write event log: {}", e);
		} else {
			println!("📝 Event log written to: {}", log_path.display());
		}
	}

	/// Clean up test resources
	async fn cleanup(self) -> Result<(), Box<dyn std::error::Error>> {
		// Dump events before cleanup
		self.dump_events().await;

		// Stop watcher
		self.watcher.stop().await?;

		// Close library
		let lib_id = self.library.id();
		self.core.libraries.close_library(lib_id).await?;
		drop(self.library);

		// Shutdown core
		self.core.shutdown().await?;

		// Remove test directory
		if self.test_dir.exists() {
			tokio::fs::remove_dir_all(&self.test_dir).await?;
		}

		println!("✓ Cleaned up test environment");
		Ok(())
	}
}

/// Inner test logic that can fail
async fn run_test_scenarios(harness: &TestHarness) -> Result<(), Box<dyn std::error::Error>> {
	// Note: Entry counts include +1 for the root directory itself which is indexed

	// ========================================================================
	// Scenario 1: Initial State
	// ========================================================================
	println!("\n--- Scenario 1: Initial State ---");
	harness.verify_entry_exists("initial.txt").await?;
	harness.verify_is_file("initial.txt").await?;
	harness.verify_entry_count(2).await?; // root dir + initial.txt

	// ========================================================================
	// Scenario 2: Create Files
	// ========================================================================
	println!("\n--- Scenario 2: Create Files ---");

	harness.create_file("document.txt", "Hello World").await?;
	harness.verify_entry_exists("document.txt").await?;
	harness.verify_is_file("document.txt").await?;

	harness
		.create_file("notes.md", "# My Notes\n\nSome content")
		.await?;
	harness.verify_entry_exists("notes.md").await?;
	harness.verify_is_file("notes.md").await?;
	harness.verify_entry_count(4).await?; // root + initial.txt, document.txt, notes.md

	// ========================================================================
	// Scenario 3: Modify Files
	// ========================================================================
	println!("\n--- Scenario 3: Modify Files ---");

	harness
		.modify_file("document.txt", "Hello World - Updated!")
		.await?;
	// File should still exist after modification
	harness.verify_entry_exists("document.txt").await?;
	harness.verify_entry_count(4).await?; // Count unchanged

	// ========================================================================
	// Scenario 4: Rename Files
	// ========================================================================
	println!("\n--- Scenario 4: Rename Files ---");

	harness.rename_file("notes.md", "notes-renamed.md").await?;
	harness.verify_entry_exists("notes-renamed.md").await?;
	harness.verify_entry_not_exists("notes.md").await?;
	harness.verify_is_file("notes-renamed.md").await?;
	harness.verify_entry_count(4).await?; // Count unchanged (rename doesn't add/remove)

	// ========================================================================
	// Scenario 5: Delete Files
	// ========================================================================
	println!("\n--- Scenario 5: Delete Files ---");

	harness.delete_file("document.txt").await?;
	harness.verify_entry_not_exists("document.txt").await?;
	harness.verify_entry_count(3).await?; // root + initial.txt, notes-renamed.md

	// ========================================================================
	// Scenario 6: Create Directory (shallow watch should detect it)
	// ========================================================================
	println!("\n--- Scenario 6: Create Directory ---");

	harness.create_dir("projects").await?;
	harness.verify_entry_exists("projects").await?;
	harness.verify_is_directory("projects").await?;
	harness.verify_entry_count(4).await?; // root + initial.txt, notes-renamed.md, projects/

	// ========================================================================
	// Final State Verification
	// ========================================================================
	println!("\n--- Final State Verification ---");
	harness.dump_index_state().await;

	// Verify exact expected final state
	harness.verify_entry_exists("initial.txt").await?;
	harness.verify_entry_exists("notes-renamed.md").await?;
	harness.verify_entry_exists("projects").await?;
	harness.verify_entry_not_exists("document.txt").await?;
	harness.verify_entry_not_exists("notes.md").await?;

	Ok(())
}

/// Comprehensive "story" test demonstrating ephemeral watcher functionality
#[tokio::test]
async fn test_ephemeral_watcher() -> Result<(), Box<dyn std::error::Error>> {
	println!("\n=== Ephemeral Watcher Full Story Test ===\n");

	let harness = TestHarness::setup().await?;

	// Run tests and capture result
	let test_result = run_test_scenarios(&harness).await;

	// ALWAYS dump events, even on failure
	harness.dump_events().await;

	// Check if test passed
	if test_result.is_err() {
		println!("\n❌ Test failed - see event log above for details");
		harness.cleanup().await?;
		return test_result;
	}

	// ========================================================================
	// Final Summary
	// ========================================================================
	println!("\n--- Test Summary ---");
	println!("✓ All tested scenarios passed!");
	println!("\nScenarios successfully tested:");
	println!("  ✓ Initial ephemeral indexing");
	println!("  ✓ File creation (immediate detection)");
	println!("  ✓ File modification (in-memory index update)");
	println!("  ✓ File renaming (ephemeral index updated)");
	println!("  ✓ File deletion (removed from ephemeral index)");
	println!("  ✓ Directory creation (shallow watch)");

	harness.cleanup().await?;

	println!("\n=== Ephemeral Watcher Test Passed ===\n");

	Ok(())
}
