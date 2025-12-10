//! Ephemeral Watcher Integration Test
//!
//! Tests the real-time file system monitoring functionality for ephemeral indexing
//! through a comprehensive "story" of file operations, verifying that the watcher
//! correctly detects and updates the in-memory ephemeral index for all filesystem changes.

use sd_core::{
	context::CoreContext,
	infra::event::Event,
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
// FsWatcher Event Collector (raw filesystem events)
// ============================================================================

/// Collects FsEvents from the watcher for diagnostic output
struct FsEventCollector {
	events: Arc<Mutex<Vec<(std::time::Instant, FsEvent)>>>,
	start_time: std::time::Instant,
}

impl FsEventCollector {
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
						eprintln!("FsEvent collector lagged by {} events", n);
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

		println!("\n=== FsWatcher Event Summary ===");
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
		println!("===============================\n");
	}
}

// ============================================================================
// Core Event Collector (ResourceChanged events from event bus)
// ============================================================================

/// Collected core event with timestamp and extracted info
struct CollectedCoreEvent {
	timestamp: std::time::Instant,
	event_type: String, // "ResourceChanged" or "ResourceDeleted"
	resource_type: String,
	// Extracted fields
	id: Option<String>,
	name: Option<String>,
	extension: Option<String>,
	content_kind: Option<String>,
	is_dir: Option<bool>,
	size: Option<u64>,
	// Full resource JSON for detailed inspection
	resource_json: String,
}

/// Collects ResourceChanged events from the Core's event bus
struct CoreEventCollector {
	events: Arc<Mutex<Vec<CollectedCoreEvent>>>,
	start_time: std::time::Instant,
}

impl CoreEventCollector {
	fn new() -> Self {
		Self {
			events: Arc::new(Mutex::new(Vec::new())),
			start_time: std::time::Instant::now(),
		}
	}

	/// Start collecting events from the core's event bus
	fn start_collecting(&self, core: &Core) {
		let events = self.events.clone();
		let mut subscriber = core.events.subscribe();

		tokio::spawn(async move {
			loop {
				match subscriber.recv().await {
					Ok(event) => {
						match event {
							Event::ResourceChanged {
								resource_type,
								resource,
								..
							} => {
								// Extract various fields from resource
								let id = resource
									.get("id")
									.and_then(|v| v.as_str())
									.map(String::from);

								let name = resource
									.get("name")
									.and_then(|v| v.as_str())
									.map(String::from);

								let extension = resource
									.get("extension")
									.and_then(|v| v.as_str())
									.map(String::from);

								let content_kind = resource
									.get("content_kind")
									.and_then(|v| v.as_str())
									.map(String::from);

								// Check if it's a directory from "kind" field
								let is_dir = resource
									.get("kind")
									.and_then(|v| v.as_str())
									.map(|k| k == "Directory");

								let size = resource.get("size").and_then(|v| v.as_u64());

								// Store compact JSON representation
								let resource_json =
									serde_json::to_string_pretty(&resource).unwrap_or_default();

								let collected = CollectedCoreEvent {
									timestamp: std::time::Instant::now(),
									event_type: "ResourceChanged".to_string(),
									resource_type,
									id,
									name,
									extension,
									content_kind,
									is_dir,
									size,
									resource_json,
								};

								let mut events_lock = events.lock().await;
								events_lock.push(collected);
							}
							Event::ResourceDeleted {
								resource_type,
								resource_id,
							} => {
								let collected = CollectedCoreEvent {
									timestamp: std::time::Instant::now(),
									event_type: "ResourceDeleted".to_string(),
									resource_type,
									id: Some(resource_id.to_string()),
									name: None,
									extension: None,
									content_kind: None,
									is_dir: None,
									size: None,
									resource_json: format!(
										"{{\"deleted_id\": \"{}\"}}",
										resource_id
									),
								};

								let mut events_lock = events.lock().await;
								events_lock.push(collected);
							}
							_ => {} // Ignore other event types
						}
					}
					Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
					Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
						eprintln!("Core event collector lagged by {} events", n);
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

		writeln!(file, "=== Core ResourceChanged Event Log ===")?;
		writeln!(file, "Total events collected: {}", events.len())?;
		writeln!(file, "")?;

		for (i, event) in events.iter().enumerate() {
			let elapsed = event.timestamp.duration_since(self.start_time);
			writeln!(file, "{}", "-".repeat(70))?;
			writeln!(
				file,
				"[{:03}] +{:.3}s | {} ({})",
				i,
				elapsed.as_secs_f64(),
				event.event_type,
				event.resource_type
			)?;
			writeln!(file, "{}", "-".repeat(70))?;

			// Summary line
			let name_str = event.name.as_deref().unwrap_or("?");
			let ext_str = event
				.extension
				.as_ref()
				.map(|e| format!(".{}", e))
				.unwrap_or_default();
			let kind_str = event.content_kind.as_deref().unwrap_or("unknown");
			let dir_str = if event.is_dir == Some(true) {
				" [DIR]"
			} else {
				""
			};
			let size_str = event
				.size
				.map(|s| format!(" ({} bytes)", s))
				.unwrap_or_default();

			writeln!(
				file,
				"  {} {}{}{}{} | kind: {}",
				if event.is_dir == Some(true) {
					"📁"
				} else {
					"📄"
				},
				name_str,
				ext_str,
				dir_str,
				size_str,
				kind_str
			)?;

			if let Some(ref id) = event.id {
				writeln!(file, "  id: {}", id)?;
			}

			// Full JSON (indented)
			writeln!(file, "\n  Full Resource JSON:")?;
			for line in event.resource_json.lines() {
				writeln!(file, "    {}", line)?;
			}
			writeln!(file, "")?;
		}

		writeln!(file, "=== End of Core Event Log ===")?;

		Ok(())
	}

	/// Print summary to console
	async fn print_summary(&self) {
		let events = self.events.lock().await;

		println!("\n=== Core Event Summary ===");
		println!("Total events: {}", events.len());

		// Count by event type
		let mut changed = 0;
		let mut deleted = 0;

		for event in events.iter() {
			match event.event_type.as_str() {
				"ResourceChanged" => changed += 1,
				"ResourceDeleted" => deleted += 1,
				_ => {}
			}
		}

		println!("  ResourceChanged: {}", changed);
		println!("  ResourceDeleted: {}", deleted);
		println!("==========================\n");
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
	fs_event_collector: FsEventCollector,
	core_event_collector: CoreEventCollector,
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

		// Create FsEvent collector and start collecting from watcher
		let fs_event_collector = FsEventCollector::new();
		fs_event_collector.start_collecting(&watcher);
		println!("✓ Started FsEvent collector");

		// Create Core event collector and start collecting from event bus
		let core_event_collector = CoreEventCollector::new();
		core_event_collector.start_collecting(&core);
		println!("✓ Started Core event collector");

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
			fs_event_collector,
			core_event_collector,
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

	/// Delete a directory recursively
	async fn delete_dir(&self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
		let path = self.path(name);
		tokio::fs::remove_dir_all(&path).await?;
		println!("Deleted directory recursively: {}", name);
		Ok(())
	}

	/// Create multiple files at the top level (batch creation test)
	async fn create_batch_files(&self, files: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
		for file in files {
			let full_path = self.path(file);
			tokio::fs::write(&full_path, format!("Content of {}", file)).await?;
			println!("Created file: {}", file);
		}
		Ok(())
	}

	/// Move a file to a location outside the watched directory (simulates trash)
	async fn move_to_trash(
		&self,
		name: &str,
		trash_dir: &std::path::Path,
	) -> Result<(), Box<dyn std::error::Error>> {
		let from_path = self.path(name);
		let to_path = trash_dir.join(name);
		tokio::fs::rename(&from_path, &to_path).await?;
		println!("Moved to trash: {} -> {}", name, to_path.display());
		Ok(())
	}

	/// Move a file back from trash (simulates undo delete / restore)
	async fn restore_from_trash(
		&self,
		name: &str,
		trash_dir: &std::path::Path,
	) -> Result<(), Box<dyn std::error::Error>> {
		let from_path = trash_dir.join(name);
		let to_path = self.path(name);
		tokio::fs::rename(&from_path, &to_path).await?;
		println!("Restored from trash: {} -> {}", from_path.display(), name);
		Ok(())
	}

	/// Create multiple directories at the top level
	async fn create_batch_dirs(&self, dirs: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
		for dir in dirs {
			self.create_dir(dir).await?;
		}
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

	/// Get children count using list_directory (like the UI does)
	/// This is the critical check - it uses the arena's children list
	async fn get_children_count(&self) -> usize {
		let index = self.context.ephemeral_cache().get_global_index();
		let index_lock = index.read().await;
		index_lock
			.list_directory(&self.test_dir)
			.map(|v| v.len())
			.unwrap_or(0)
	}

	/// Verify children count using list_directory (catches the orphan bug)
	async fn verify_children_count(
		&self,
		expected: usize,
	) -> Result<(), Box<dyn std::error::Error>> {
		let count = self.get_children_count().await;
		if count == expected {
			println!("✓ Children count (list_directory) matches: {}", expected);
			Ok(())
		} else {
			Err(format!(
				"Children count mismatch: expected {} but found {} (using list_directory)",
				expected, count
			)
			.into())
		}
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

	/// Dump collected events to files and print summaries
	async fn dump_events(&self) {
		// Print FsWatcher event summary
		self.fs_event_collector.print_summary().await;

		// Print Core event summary
		self.core_event_collector.print_summary().await;

		// Write FsWatcher events to file
		let fs_log_path = std::env::temp_dir().join("ephemeral_watcher_fs_events.log");
		if let Err(e) = self.fs_event_collector.dump_to_file(&fs_log_path).await {
			eprintln!("Failed to write FsEvent log: {}", e);
		} else {
			println!("📝 FsEvent log written to: {}", fs_log_path.display());
		}

		// Write Core events to file
		let core_log_path = std::env::temp_dir().join("ephemeral_watcher_core_events.log");
		if let Err(e) = self.core_event_collector.dump_to_file(&core_log_path).await {
			eprintln!("Failed to write Core event log: {}", e);
		} else {
			println!("📝 Core event log written to: {}", core_log_path.display());
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
	// Scenario 7: Batch Create Files and Directories
	// ========================================================================
	println!("\n--- Scenario 7: Batch Create Files and Directories ---");

	// Create multiple files at once (simulating drag-and-drop or copy operations)
	let batch_files = [
		"readme.txt",
		"config.json",
		"data.csv",
		"report.md",
		"script.sh",
		"image.png",
	];
	harness.create_batch_files(&batch_files).await?;

	// Create multiple directories at once
	// Note: Avoid names like "temp", "cache", etc. that may be filtered by indexing rules
	let batch_dirs = ["workspace", "backups", "archives"];
	harness.create_batch_dirs(&batch_dirs).await?;

	// Give the watcher time to process all the create events
	tokio::time::sleep(Duration::from_millis(500)).await;

	// Verify all batch-created files appear
	for file in &batch_files {
		harness.verify_entry_exists(file).await?;
		harness.verify_is_file(file).await?;
	}

	// Verify all batch-created directories appear
	for dir in &batch_dirs {
		harness.verify_entry_exists(dir).await?;
		harness.verify_is_directory(dir).await?;
	}

	// Count: root(1) + initial.txt(1) + notes-renamed.md(1) + projects(1) +
	//        6 files + 3 dirs = 13
	harness.verify_entry_count(13).await?;

	println!("✓ All batch-created entries verified in index");
	harness.dump_index_state().await;

	// ========================================================================
	// Scenario 8: Delete Multiple Files and Directory
	// ========================================================================
	println!("\n--- Scenario 8: Delete Multiple Files and Directory ---");

	// Delete multiple files
	for file in &batch_files {
		harness.delete_file(file).await?;
	}

	// Delete directories (they're empty at top level, so just rmdir)
	for dir in &batch_dirs {
		harness.delete_dir(dir).await?;
	}

	// Give the watcher time to process all the delete events
	tokio::time::sleep(Duration::from_millis(500)).await;

	// Verify all batch-created entries are removed
	for file in &batch_files {
		harness.verify_entry_not_exists(file).await?;
	}
	for dir in &batch_dirs {
		harness.verify_entry_not_exists(dir).await?;
	}

	// Count should be back to: root(1) + initial.txt(1) + notes-renamed.md(1) + projects(1) = 4
	harness.verify_entry_count(4).await?;

	println!("✓ All batch-deleted entries removed from index");

	// ========================================================================
	// Scenario 9: Delete + Undo (Restore) - Tests for duplicate entry bug
	// ========================================================================
	println!("\n--- Scenario 9: Delete + Undo (Restore) Pattern ---");
	println!("This tests for the duplicate entry bug when files are restored after deletion");

	// Create a temporary "trash" directory outside the watched directory
	let trash_dir = std::env::temp_dir().join("sd_test_trash");
	if trash_dir.exists() {
		tokio::fs::remove_dir_all(&trash_dir).await?;
	}
	tokio::fs::create_dir_all(&trash_dir).await?;
	println!("Created trash directory: {}", trash_dir.display());

	// Create files to test delete + restore
	let restore_files = [
		"restore_test1.txt",
		"restore_test2.txt",
		"restore_test3.txt",
	];
	harness.create_batch_files(&restore_files).await?;

	// Wait for creates to be processed
	tokio::time::sleep(Duration::from_millis(500)).await;

	// Verify files are in the index
	for file in &restore_files {
		harness.verify_entry_exists(file).await?;
	}

	// Record entry count before delete (using both methods)
	let count_before_delete = harness.get_entry_count().await;
	let children_before_delete = harness.get_children_count().await;
	println!("Entry count before delete: {}", count_before_delete);
	println!(
		"Children count (list_directory) before delete: {}",
		children_before_delete
	);
	harness.dump_index_state().await;

	// "Delete" files by moving to trash (simulates Finder trash)
	println!("\nMoving files to trash (simulating delete)...");
	for file in &restore_files {
		harness.move_to_trash(file, &trash_dir).await?;
	}

	// Wait for deletes to be processed
	tokio::time::sleep(Duration::from_millis(500)).await;

	// Verify files are removed from index
	for file in &restore_files {
		harness.verify_entry_not_exists(file).await?;
	}

	let count_after_delete = harness.get_entry_count().await;
	let children_after_delete = harness.get_children_count().await;
	println!("Entry count after delete: {}", count_after_delete);
	println!(
		"Children count (list_directory) after delete: {}",
		children_after_delete
	);
	harness.dump_index_state().await;

	// Verify count decreased by the number of deleted files
	assert_eq!(
		count_after_delete,
		count_before_delete - restore_files.len(),
		"Entry count should decrease by {} after delete",
		restore_files.len()
	);

	// CRITICAL: Also verify children count using list_directory (like UI does)
	assert_eq!(
		children_after_delete,
		children_before_delete - restore_files.len(),
		"ORPHAN BUG: Children count (list_directory) should decrease by {} after delete. \
		 path_index shows {} but list_directory shows {}",
		restore_files.len(),
		count_after_delete,
		children_after_delete
	);

	// "Restore" files by moving back from trash (simulates Undo delete)
	println!("\nRestoring files from trash (simulating undo delete)...");
	for file in &restore_files {
		harness.restore_from_trash(file, &trash_dir).await?;
	}

	// Wait for restores to be processed
	tokio::time::sleep(Duration::from_millis(500)).await;

	// Verify files are back in the index
	for file in &restore_files {
		harness.verify_entry_exists(file).await?;
	}

	let count_after_restore = harness.get_entry_count().await;
	let children_after_restore = harness.get_children_count().await;
	println!("Entry count after restore: {}", count_after_restore);
	println!(
		"Children count (list_directory) after restore: {}",
		children_after_restore
	);
	harness.dump_index_state().await;

	// THIS IS THE KEY ASSERTION - count should match the original
	// If there's a duplicate entry bug, count_after_restore will be higher
	assert_eq!(
		count_after_restore, count_before_delete,
		"DUPLICATE ENTRY BUG DETECTED: Entry count after restore ({}) should equal count before delete ({}). \
		 Expected {} entries but found {}. This indicates files were added without removing stale entries.",
		count_after_restore,
		count_before_delete,
		count_before_delete,
		count_after_restore
	);

	// CRITICAL: Also verify children count (this catches the orphan bug the UI sees)
	assert_eq!(
		children_after_restore, children_before_delete,
		"ORPHAN BUG IN list_directory: Children count after restore ({}) should equal before delete ({}). \
		 path_index shows {} but list_directory shows {}. This is the bug you saw in the UI!",
		children_after_restore,
		children_before_delete,
		count_after_restore,
		children_after_restore
	);

	println!("✓ Delete + Restore pattern: entry count is correct (no duplicates)");
	println!("✓ Delete + Restore pattern: children count (list_directory) is correct");

	// Clean up trash directory
	tokio::fs::remove_dir_all(&trash_dir).await?;

	// ========================================================================
	// Scenario 10: Screenshot Pattern (rapid create + write)
	// ========================================================================
	println!("\n--- Scenario 10: Screenshot Pattern (rapid create + write) ---");
	println!("This tests for duplicate entries when file is created then immediately written");

	let screenshot_file = "screenshot_test.png";
	let screenshot_path = harness.path(screenshot_file);

	// Record counts before
	let children_before_screenshot = harness.get_children_count().await;
	println!(
		"Children count before screenshot: {}",
		children_before_screenshot
	);

	// Simulate screenshot behavior (aggressive version):
	// macOS creates file, writes header, then writes full data in rapid succession
	// We'll do multiple rapid writes to try to trigger race conditions

	// 1. Create with tiny content
	tokio::fs::write(&screenshot_path, b"x").await?;
	println!("Created with tiny content: {}", screenshot_file);

	// 2. Immediate overwrites (no delay - trying to trigger race)
	tokio::fs::write(&screenshot_path, b"xx").await?;
	tokio::fs::write(&screenshot_path, b"xxx").await?;

	// 3. Write "header"
	let fake_png_header = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
	tokio::fs::write(&screenshot_path, &fake_png_header).await?;
	println!("Wrote header ({} bytes)", fake_png_header.len());

	// 4. Write full content (final state)
	let mut full_content = fake_png_header.clone();
	full_content.extend(vec![0u8; 50000]); // Simulate ~50KB image
	tokio::fs::write(&screenshot_path, &full_content).await?;
	println!(
		"Wrote full content ({} bytes): {}",
		full_content.len(),
		screenshot_file
	);

	// Wait for watcher to process
	tokio::time::sleep(Duration::from_millis(500)).await;

	// Verify only ONE entry exists
	harness.verify_entry_exists(screenshot_file).await?;

	let children_after_screenshot = harness.get_children_count().await;
	println!(
		"Children count after screenshot: {}",
		children_after_screenshot
	);

	// Should have exactly ONE more entry (the screenshot file)
	assert_eq!(
		children_after_screenshot,
		children_before_screenshot + 1,
		"DUPLICATE ENTRY BUG: Screenshot pattern created {} entries instead of 1. \
		 Expected {} but got {} children",
		children_after_screenshot - children_before_screenshot,
		children_before_screenshot + 1,
		children_after_screenshot
	);

	println!("✓ Screenshot pattern: only one entry created (no duplicates)");

	// Clean up
	harness.delete_file(screenshot_file).await?;
	tokio::time::sleep(Duration::from_millis(500)).await;

	// Clean up test files for final state
	for file in &restore_files {
		harness.delete_file(file).await?;
	}
	tokio::time::sleep(Duration::from_millis(500)).await;

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
	// All batch-created items should be gone
	harness.verify_entry_not_exists("workspace").await?;
	harness.verify_entry_not_exists("readme.txt").await?;

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
	println!("  ✓ Batch file/directory creation");
	println!("  ✓ Batch file/directory deletion");

	harness.cleanup().await?;

	println!("\n=== Ephemeral Watcher Test Passed ===\n");

	Ok(())
}
