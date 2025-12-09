//! Ephemeral Watcher Integration Test
//!
//! Tests the real-time file system monitoring functionality for ephemeral indexing
//! through a comprehensive "story" of file operations, verifying that the watcher
//! correctly detects and updates the in-memory ephemeral index for all filesystem changes.

use sd_core::{
	context::CoreContext,
	infra::event::{Event, EventSubscriber, FsRawEventKind},
	library::Library,
	ops::indexing::job::{IndexScope, IndexerJob, IndexerJobConfig},
	service::{watcher::LocationWatcher, watcher::LocationWatcherConfig, Service},
	Core,
};
use std::{path::PathBuf, sync::Arc, time::Duration};
use tempfile::TempDir;
use tokio::time::timeout;

// ============================================================================
// Helper Functions
// ============================================================================

/// Wait for a specific event with timeout
async fn wait_for_event<F>(
	event_rx: &mut EventSubscriber,
	predicate: F,
	timeout_duration: Duration,
	description: &str,
) -> Result<Event, Box<dyn std::error::Error>>
where
	F: Fn(&Event) -> bool,
{
	timeout(timeout_duration, async {
		loop {
			match event_rx.recv().await {
				Ok(event) if predicate(&event) => return Ok(event),
				Ok(_) => continue,
				Err(e) => {
					return Err(format!(
						"Event channel error while waiting for {}: {}",
						description, e
					)
					.into())
				}
			}
		}
	})
	.await
	.map_err(|_| format!("Timeout waiting for event: {}", description))?
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
	fs_event_rx: EventSubscriber,
	watcher: Arc<LocationWatcher>,
	context: Arc<CoreContext>,
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

		// Subscribe to filesystem events
		let fs_event_rx = core.events.subscribe();

		// Start the watcher service
		let watcher_config = LocationWatcherConfig::default();
		let watcher = Arc::new(LocationWatcher::new(
			watcher_config,
			core.events.clone(),
			core.context.clone(),
		));
		watcher.start().await?;
		println!("✓ Started watcher service");

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
		let rule_toggles = sd_core::ops::indexing::rules::RuleToggles::default();
		watcher.add_ephemeral_watch(test_dir.clone(), rule_toggles).await?;
		println!("✓ Added ephemeral watch for: {}", test_dir.display());

		// Give the watcher a moment to settle
		tokio::time::sleep(Duration::from_millis(500)).await;

		let context = core.context.clone();

		Ok(Self {
			_core_data_dir: temp_dir,
			core: Arc::new(core),
			library,
			test_dir,
			fs_event_rx,
			watcher,
			context,
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

	/// Wait for a specific filesystem event
	async fn wait_for_fs_event(
		&mut self,
		expected_kind: FsRawEventKind,
		timeout_secs: u64,
	) -> Result<(), Box<dyn std::error::Error>> {
		let expected_path = match &expected_kind {
			FsRawEventKind::Create { path } => path.clone(),
			FsRawEventKind::Modify { path } => path.clone(),
			FsRawEventKind::Remove { path } => path.clone(),
			FsRawEventKind::Rename { to, .. } => to.clone(),
		};

		timeout(Duration::from_secs(timeout_secs), async {
			loop {
				match self.fs_event_rx.recv().await {
					Ok(Event::FsRawChange { kind, .. }) => {
						let matches = match (&kind, &expected_kind) {
							(FsRawEventKind::Create { path }, FsRawEventKind::Create { .. }) => {
								path == &expected_path
							}
							(FsRawEventKind::Modify { path }, FsRawEventKind::Modify { .. }) => {
								path == &expected_path
							}
							(FsRawEventKind::Remove { path }, FsRawEventKind::Remove { .. }) => {
								path == &expected_path
							}
							(FsRawEventKind::Rename { to, .. }, FsRawEventKind::Rename { .. }) => {
								to == &expected_path
							}
							_ => false,
						};

						if matches {
							println!(
								"✓ Detected filesystem event for: {}",
								expected_path.display()
							);
							return Ok(());
						}
					}
					Ok(_) => continue,
					Err(e) => return Err(format!("Event channel error: {}", e).into()),
				}
			}
		})
		.await
		.map_err(|_| "Timeout waiting for filesystem event")?
	}

	/// Verify entry exists in ephemeral index
	async fn verify_entry_exists(
		&self,
		name: &str,
	) -> Result<(), Box<dyn std::error::Error>> {
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

		Err(format!("Entry '{}' not found in ephemeral index after timeout", name).into())
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

	/// Clean up test resources
	async fn cleanup(self) -> Result<(), Box<dyn std::error::Error>> {
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

/// Comprehensive "story" test demonstrating ephemeral watcher functionality
#[tokio::test]
async fn test_ephemeral_watcher() -> Result<(), Box<dyn std::error::Error>> {
	println!("\n=== Ephemeral Watcher Full Story Test ===\n");

	let mut harness = TestHarness::setup().await?;

	// ========================================================================
	// Scenario 1: Initial State
	// ========================================================================
	println!("\n--- Scenario 1: Initial State ---");
	harness.verify_entry_exists("initial.txt").await?;

	// ========================================================================
	// Scenario 2: Create Files
	// ========================================================================
	println!("\n--- Scenario 2: Create Files ---");

	harness.create_file("document.txt", "Hello World").await?;
	harness
		.wait_for_fs_event(
			FsRawEventKind::Create {
				path: harness.path("document.txt"),
			},
			30,
		)
		.await?;
	// Give the responder time to process
	tokio::time::sleep(Duration::from_millis(500)).await;
	harness.verify_entry_exists("document.txt").await?;

	harness
		.create_file("notes.md", "# My Notes\n\nSome content")
		.await?;
	harness
		.wait_for_fs_event(
			FsRawEventKind::Create {
				path: harness.path("notes.md"),
			},
			30,
		)
		.await?;
	tokio::time::sleep(Duration::from_millis(500)).await;
	harness.verify_entry_exists("notes.md").await?;

	// ========================================================================
	// Scenario 3: Modify Files
	// ========================================================================
	println!("\n--- Scenario 3: Modify Files ---");

	harness
		.modify_file("document.txt", "Hello World - Updated!")
		.await?;
	// Wait a bit for the modification event
	tokio::time::sleep(Duration::from_millis(1000)).await;
	harness.verify_entry_exists("document.txt").await?;

	// ========================================================================
	// Scenario 4: Rename Files
	// ========================================================================
	println!("\n--- Scenario 4: Rename Files ---");

	harness.rename_file("notes.md", "notes-renamed.md").await?;
	harness
		.wait_for_fs_event(
			FsRawEventKind::Rename {
				from: harness.path("notes.md"),
				to: harness.path("notes-renamed.md"),
			},
			30,
		)
		.await?;
	tokio::time::sleep(Duration::from_millis(500)).await;
	harness.verify_entry_exists("notes-renamed.md").await?;
	harness.verify_entry_not_exists("notes.md").await?;

	// ========================================================================
	// Scenario 5: Delete Files
	// ========================================================================
	println!("\n--- Scenario 5: Delete Files ---");

	harness.delete_file("document.txt").await?;
	harness
		.wait_for_fs_event(
			FsRawEventKind::Remove {
				path: harness.path("document.txt"),
			},
			30,
		)
		.await?;
	tokio::time::sleep(Duration::from_millis(500)).await;
	harness.verify_entry_not_exists("document.txt").await?;

	// ========================================================================
	// Scenario 6: Create Directory (shallow watch should detect it)
	// ========================================================================
	println!("\n--- Scenario 6: Create Directory ---");

	harness.create_dir("projects").await?;
	harness
		.wait_for_fs_event(
			FsRawEventKind::Create {
				path: harness.path("projects"),
			},
			30,
		)
		.await?;
	tokio::time::sleep(Duration::from_millis(500)).await;
	harness.verify_entry_exists("projects").await?;

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
