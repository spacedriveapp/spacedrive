//! Test to reproduce ghost folder bug when moving folders into managed locations
//!
//! This test reproduces the issue where moving a folder into a managed location
//! triggers a reindex that creates duplicate entries with wrong parent_ids.
//!
//! ## Bug Description
//! When a folder is moved from outside a managed location into it (e.g., moving
//! Desk1 into Desktop), the watcher triggers a reindex at that subpath. During
//! this reindex, entries are created with incorrect parent_id values, pointing
//! to the location root instead of their actual parent directory.
//!
//! ## Expected Behavior
//! - Desk/ moved into Desktop/
//! - Desk/Subfolder/ should have parent_id = Desk1's entry ID
//!
//! ## Actual Behavior
//! - Desk/Subfolder/ gets parent_id = Desktop's entry ID (wrong!)
//! - Creates "ghost folders" that appear at Desktop root in API but don't exist there
//!
//! ## Running Test
//! ```bash
//! cargo test -p sd-core --test indexing_move_folder_bug_test -- --nocapture
//! ```

// mod helpers; // Disabled due to compile errors in sync helper

use sd_core::{
	infra::{db::entities, event::Event},
	location::{create_location, IndexMode, LocationCreateArgs},
	Core,
};
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
use std::{path::PathBuf, sync::Arc};
use tokio::{fs, sync::Mutex, time::Duration};
use uuid::Uuid;

struct TestHarness {
	test_root: PathBuf,
	library: Arc<sd_core::library::Library>,
	event_log: Arc<Mutex<Vec<Event>>>,
	snapshot_dir: PathBuf,
}

impl TestHarness {
	async fn new(test_name: &str) -> anyhow::Result<Self> {
		// Create test root
		let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
		let test_root =
			PathBuf::from(home).join("Library/Application Support/spacedrive/indexing_bug_tests");

		// Create data directory for spacedrive
		let data_dir = test_root.join("data");
		fs::create_dir_all(&data_dir).await?;

		// Create snapshot directory
		let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
		let snapshot_dir = test_root
			.join("snapshots")
			.join(format!("{}_{}", test_name, timestamp));
		fs::create_dir_all(&snapshot_dir).await?;

		// Initialize logging
		let log_file = std::fs::File::create(snapshot_dir.join("test.log"))?;
		use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

		let _ = tracing_subscriber::registry()
			.with(
				fmt::layer()
					.with_target(true)
					.with_thread_ids(true)
					.with_ansi(false)
					.with_writer(log_file),
			)
			.with(fmt::layer().with_target(true).with_thread_ids(true))
			.with(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
				EnvFilter::new(
					"sd_core::ops::indexing=debug,\
						 sd_core::ops::indexing::entry=trace,\
						 sd_core::ops::indexing::responder=trace,\
						 sd_core::location=debug,\
						 indexing_move_folder_bug_test=debug",
				)
			}))
			.try_init();

		tracing::info!(
			test_root = %test_root.display(),
			snapshot_dir = %snapshot_dir.display(),
			"Created test environment"
		);

		// Create config
		Self::create_test_config(&data_dir)?;

		// Initialize core
		let core = Core::new(data_dir.clone())
			.await
			.map_err(|e| anyhow::anyhow!("Failed to create core: {}", e))?;

		// Create library
		let library = core
			.libraries
			.create_library_no_sync("Bug Reproduction Library", None, core.context.clone())
			.await?;

		// Set up event collection
		let event_log = Arc::new(Mutex::new(Vec::new()));
		Self::start_event_collector(&library, event_log.clone());

		Ok(Self {
			test_root,
			library,
			event_log,
			snapshot_dir,
		})
	}

	fn create_test_config(
		data_dir: &std::path::Path,
	) -> anyhow::Result<sd_core::config::AppConfig> {
		let config = sd_core::config::AppConfig {
			version: 4,
			logging: sd_core::config::LoggingConfig {
				main_filter: "sd_core=debug".to_string(),
				streams: vec![],
			},
			data_dir: data_dir.to_path_buf(),
			log_level: "debug".to_string(),
			telemetry_enabled: false,
			preferences: sd_core::config::Preferences::default(),
			job_logging: sd_core::config::JobLoggingConfig::default(),
			services: sd_core::config::ServiceConfig {
				networking_enabled: false,
				volume_monitoring_enabled: false,
				fs_watcher_enabled: true, // Need watcher to trigger reindex on move
				statistics_listener_enabled: false,
			},
		};

		config.save()?;
		Ok(config)
	}

	fn start_event_collector(
		library: &Arc<sd_core::library::Library>,
		event_log: Arc<Mutex<Vec<Event>>>,
	) {
		let mut subscriber = library.event_bus().subscribe();

		tokio::spawn(async move {
			while let Ok(event) = subscriber.recv().await {
				event_log.lock().await.push(event);
			}
		});
	}

	/// Create a test folder structure outside the managed location
	async fn create_test_folder_structure(&self, base_path: &PathBuf) -> anyhow::Result<()> {
		// Create folder structure: TestFolder/SubFolder1/file1.txt, SubFolder2/file2.txt
		let test_folder = base_path.join("TestFolder");
		fs::create_dir_all(&test_folder).await?;

		let subfolder1 = test_folder.join("SubFolder1");
		fs::create_dir_all(&subfolder1).await?;
		fs::write(subfolder1.join("file1.txt"), b"test content 1").await?;
		fs::write(subfolder1.join("file2.txt"), b"test content 2").await?;

		let subfolder2 = test_folder.join("SubFolder2");
		fs::create_dir_all(&subfolder2).await?;
		fs::write(subfolder2.join("file3.txt"), b"test content 3").await?;
		fs::write(subfolder2.join("file4.txt"), b"test content 4").await?;

		// Also add a file at TestFolder root
		fs::write(test_folder.join("root_file.txt"), b"root content").await?;

		tracing::info!(
			test_folder = %test_folder.display(),
			"Created test folder structure"
		);

		Ok(())
	}

	/// Add location and wait for initial indexing
	async fn add_location(&self, path: &str, name: &str) -> anyhow::Result<(Uuid, i32)> {
		tracing::info!(path = %path, name = %name, "Creating location");

		// Get device record
		let device_record = entities::device::Entity::find()
			.one(self.library.db().conn())
			.await?
			.ok_or_else(|| anyhow::anyhow!("Device not found"))?;

		// Create location (use Shallow to avoid thumbnail jobs)
		let location_args = LocationCreateArgs {
			path: PathBuf::from(path),
			name: Some(name.to_string()),
			index_mode: IndexMode::Shallow,
		};

		let location_db_id = create_location(
			self.library.clone(),
			self.library.event_bus(),
			location_args,
			device_record.id,
		)
		.await?;

		// Get location UUID
		let location_record = entities::location::Entity::find_by_id(location_db_id)
			.one(self.library.db().conn())
			.await?
			.ok_or_else(|| anyhow::anyhow!("Location not found"))?;

		tracing::info!(
			location_uuid = %location_record.uuid,
			location_id = location_db_id,
			"Location created, waiting for indexing"
		);

		// Wait for initial indexing
		self.wait_for_indexing().await?;

		Ok((location_record.uuid, location_db_id))
	}

	/// Wait for indexing jobs to complete (ignores thumbnail/processor jobs)
	async fn wait_for_indexing(&self) -> anyhow::Result<()> {
		use sd_core::infra::job::JobStatus;

		// Just wait a bit for jobs to start and complete
		tokio::time::sleep(Duration::from_millis(500)).await;

		let start_time = tokio::time::Instant::now();
		let timeout = Duration::from_secs(10);

		loop {
			let all_running = self
				.library
				.jobs()
				.list_jobs(Some(JobStatus::Running))
				.await?;

			// Filter to only indexer jobs (ignore thumbnail/processor jobs)
			let indexer_jobs: Vec<_> = all_running
				.iter()
				.filter(|j| j.name.contains("indexer"))
				.collect();

			if indexer_jobs.is_empty() {
				// No indexer jobs running - we're done
				let entry_count = entities::entry::Entity::find()
					.count(self.library.db().conn())
					.await?;
				tracing::info!(entries = entry_count, "Indexing complete");
				return Ok(());
			}

			if start_time.elapsed() > timeout {
				anyhow::bail!(
					"Indexing timeout - {} indexer jobs still running",
					indexer_jobs.len()
				);
			}

			tokio::time::sleep(Duration::from_millis(200)).await;
		}
	}

	/// Capture snapshot for post-mortem analysis
	async fn capture_snapshot(&self, phase: &str) -> anyhow::Result<()> {
		let phase_dir = self.snapshot_dir.join(phase);
		fs::create_dir_all(&phase_dir).await?;

		tracing::info!(phase = %phase, path = %phase_dir.display(), "Capturing snapshot");

		// Copy database
		let src_db = self.library.path().join("database.db");
		let dest_db = phase_dir.join("database.db");
		if src_db.exists() {
			fs::copy(&src_db, &dest_db).await?;
		}

		// Write event log
		let events = self.event_log.lock().await;
		let mut event_file = fs::File::create(phase_dir.join("events.log")).await?;
		use tokio::io::AsyncWriteExt;
		for event in events.iter() {
			let line = format!("{}\n", serde_json::to_string(event)?);
			event_file.write_all(line.as_bytes()).await?;
		}

		// Write database analysis
		self.write_db_analysis(&phase_dir).await?;

		tracing::info!("Snapshot captured");
		Ok(())
	}

	/// Analyze database state and write report
	async fn write_db_analysis(&self, dest_dir: &PathBuf) -> anyhow::Result<()> {
		let mut report = String::new();
		report.push_str("# Database Analysis Report\n\n");

		// Count entries
		let total_entries = entities::entry::Entity::find()
			.count(self.library.db().conn())
			.await?;
		report.push_str(&format!("Total entries: {}\n\n", total_entries));

		// Check for duplicate names with different parents
		let conn = self.library.db().conn();

		// Get all directory entries
		let dirs = entities::entry::Entity::find()
			.filter(entities::entry::Column::Kind.eq(1))
			.all(conn)
			.await?;

		report.push_str("## Directory Entries\n\n");
		for dir in &dirs {
			// Get full path from directory_paths
			let dir_path = entities::directory_paths::Entity::find()
				.filter(entities::directory_paths::Column::EntryId.eq(dir.id))
				.one(conn)
				.await?;

			let path_str = dir_path
				.map(|dp| dp.path)
				.unwrap_or_else(|| "<no path>".to_string());

			report.push_str(&format!(
				"- ID: {}, Name: '{}', Parent ID: {:?}, Path: {}\n",
				dir.id, dir.name, dir.parent_id, path_str
			));
		}

		// Check for inconsistencies
		report.push_str("\n## Inconsistency Check\n\n");

		for dir in &dirs {
			if let Some(parent_id) = dir.parent_id {
				// Get parent's path
				let parent_path = entities::directory_paths::Entity::find()
					.filter(entities::directory_paths::Column::EntryId.eq(parent_id))
					.one(conn)
					.await?;

				// Get this dir's path
				let dir_path = entities::directory_paths::Entity::find()
					.filter(entities::directory_paths::Column::EntryId.eq(dir.id))
					.one(conn)
					.await?;

				if let (Some(parent_path), Some(dir_path)) = (parent_path, dir_path) {
					// Check if dir_path actually starts with parent_path
					let dir_pathbuf = PathBuf::from(&dir_path.path);
					let parent_pathbuf = PathBuf::from(&parent_path.path);

					if let Some(actual_parent) = dir_pathbuf.parent() {
						if actual_parent != parent_pathbuf {
							report.push_str(&format!(
								"INCONSISTENCY: Entry '{}' (ID: {})\n",
								dir.name, dir.id
							));
							report.push_str(&format!(
								"   - parent_id points to: {} ({})\n",
								parent_id, parent_path.path
							));
							report
								.push_str(&format!("   - But actual path is: {}\n", dir_path.path));
							report.push_str(&format!(
								"   - Actual parent should be: {}\n\n",
								actual_parent.display()
							));
						}
					}
				}
			}
		}

		// Check for duplicate entries
		report.push_str("\n## Duplicate Entry Check\n\n");
		let all_entries = entities::entry::Entity::find().all(conn).await?;
		let mut name_counts: std::collections::HashMap<String, Vec<i32>> =
			std::collections::HashMap::new();

		for entry in &all_entries {
			name_counts
				.entry(entry.name.clone())
				.or_insert_with(Vec::new)
				.push(entry.id);
		}

		for (name, ids) in name_counts.iter() {
			if ids.len() > 1 {
				report.push_str(&format!(
					"️  Duplicate name '{}': {} entries with IDs {:?}\n",
					name,
					ids.len(),
					ids
				));
			}
		}

		// Write report
		fs::write(dest_dir.join("analysis.md"), report.as_bytes()).await?;

		Ok(())
	}
}

/// Test: Move folder into managed location and check for ghost entries
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_move_folder_creates_ghost_entries() -> anyhow::Result<()> {
	let harness = TestHarness::new("move_folder_bug").await?;

	// Clean up from previous test runs
	let managed_location_path = harness.test_root.join("ManagedLocation");
	let outside_path = harness.test_root.join("outside");
	if managed_location_path.exists() {
		let _ = fs::remove_dir_all(&managed_location_path).await;
	}
	if outside_path.exists() {
		let _ = fs::remove_dir_all(&outside_path).await;
	}

	// Phase 1: Create a managed location at test_root/ManagedLocation
	tracing::info!("=== Phase 1: Create managed location ===");
	fs::create_dir_all(&managed_location_path).await?;

	let (_location_uuid, _location_id) = harness
		.add_location(managed_location_path.to_str().unwrap(), "ManagedLocation")
		.await?;

	// Capture initial state
	harness
		.capture_snapshot("01_after_location_creation")
		.await?;

	// Phase 2: Create test folder structure OUTSIDE the managed location
	tracing::info!("=== Phase 2: Create test folder outside managed location ===");
	let outside_path = harness.test_root.join("outside");
	fs::create_dir_all(&outside_path).await?;
	harness.create_test_folder_structure(&outside_path).await?;

	// Phase 3: Move the folder INTO the managed location
	tracing::info!("=== Phase 3: Move folder into managed location ===");
	let source = outside_path.join("TestFolder");
	let destination = managed_location_path.join("TestFolder");

	tracing::info!(
		from = %source.display(),
		to = %destination.display(),
		"Moving folder"
	);

	// Use fs::rename to simulate moving the folder
	fs::rename(&source, &destination).await?;

	tracing::info!("Folder moved, waiting for watcher to detect and trigger reindex");

	// Phase 4: Wait for watcher to detect and reindex
	// The watcher should trigger a reindex at the TestFolder subpath
	// Give watcher time to detect the new folder and spawn indexer job
	tokio::time::sleep(Duration::from_secs(3)).await;

	// Wait for any indexer jobs triggered by the watcher
	harness.wait_for_indexing().await?;

	// Give it a bit more time to ensure all processing is complete
	tokio::time::sleep(Duration::from_millis(500)).await;

	// Capture final state
	harness
		.capture_snapshot("02_after_move_and_reindex")
		.await?;

	// Phase 5: Verify database integrity
	tracing::info!("=== Phase 5: Verify database integrity ===");

	let conn = harness.library.db().conn();

	// Get all entries
	let all_entries = entities::entry::Entity::find().all(conn).await?;
	tracing::info!(total_entries = all_entries.len(), "Total entries found");

	// Check for TestFolder
	let test_folder_entry = entities::entry::Entity::find()
		.filter(entities::entry::Column::Name.eq("TestFolder"))
		.one(conn)
		.await?
		.expect("TestFolder should exist");

	tracing::info!(
		test_folder_id = test_folder_entry.id,
		test_folder_parent = ?test_folder_entry.parent_id,
		"Found TestFolder entry"
	);

	// Check SubFolder1 and SubFolder2
	let subfolder1 = entities::entry::Entity::find()
		.filter(entities::entry::Column::Name.eq("SubFolder1"))
		.one(conn)
		.await?
		.expect("SubFolder1 should exist");

	let subfolder2 = entities::entry::Entity::find()
		.filter(entities::entry::Column::Name.eq("SubFolder2"))
		.one(conn)
		.await?
		.expect("SubFolder2 should exist");

	tracing::info!(
		subfolder1_id = subfolder1.id,
		subfolder1_parent = ?subfolder1.parent_id,
		subfolder2_id = subfolder2.id,
		subfolder2_parent = ?subfolder2.parent_id,
		"Found subfolder entries"
	);

	// CRITICAL ASSERTION: SubFolder1 and SubFolder2 should have TestFolder as parent
	assert_eq!(
		subfolder1.parent_id,
		Some(test_folder_entry.id),
		"SubFolder1 should have TestFolder (ID: {}) as parent, but has {:?}",
		test_folder_entry.id,
		subfolder1.parent_id
	);

	assert_eq!(
		subfolder2.parent_id,
		Some(test_folder_entry.id),
		"SubFolder2 should have TestFolder (ID: {}) as parent, but has {:?}",
		test_folder_entry.id,
		subfolder2.parent_id
	);

	// Verify no duplicate entries
	let mut name_counts: std::collections::HashMap<String, usize> =
		std::collections::HashMap::new();
	for entry in &all_entries {
		*name_counts.entry(entry.name.clone()).or_insert(0) += 1;
	}

	for (name, count) in name_counts.iter() {
		if *count > 1 {
			tracing::error!(name = %name, count = count, "Found duplicate entries");
		}
		assert_eq!(
			*count, 1,
			"Entry '{}' appears {} times (should be 1)",
			name, count
		);
	}

	tracing::info!("All assertions passed - no ghost entries created");

	Ok(())
}
