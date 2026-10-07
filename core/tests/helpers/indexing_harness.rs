//! Indexing test harness and utilities
//!
//! Stands up a core with one library and a registered device, so volume
//! index and watcher tests can walk temp directories, collect events, and
//! assert on what the index reports.

use super::{init_test_tracing, register_device, TestConfigBuilder, TestDataDir};
use anyhow::Context;
use sd_core::{
	domain::addressing::SdPath,
	infra::{action::LibraryAction, db::entities, job::types::JobId},
	ops::{
		indexing::{IndexScope, IndexerJob, IndexerJobConfig},
		sources::track::{TrackSourceAction, TrackSourceInput, TrackSourceOutput},
	},
	Core,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use std::{
	path::{Path, PathBuf},
	sync::Arc,
};
use tokio::time::Duration;
use uuid::Uuid;

/// Builder for creating indexing test harness
pub struct IndexingHarnessBuilder {
	test_name: String,
	watcher_enabled: bool,
	daemon_enabled: bool,
}

impl IndexingHarnessBuilder {
	/// Create a new harness builder
	pub fn new(test_name: impl Into<String>) -> Self {
		Self {
			test_name: test_name.into(),
			watcher_enabled: true, // Enabled by default
			daemon_enabled: false, // Disabled by default (only for TypeScript bridge tests)
		}
	}

	/// Disable the filesystem watcher for this test
	pub fn disable_watcher(mut self) -> Self {
		self.watcher_enabled = false;
		self
	}

	/// Enable daemon RPC server for TypeScript bridge tests
	pub fn enable_daemon(mut self) -> Self {
		self.daemon_enabled = true;
		self
	}

	/// Build the harness
	pub async fn build(self) -> anyhow::Result<IndexingHarness> {
		// Use TestDataDir with watcher support (uses home directory for macOS compatibility)
		let test_data = TestDataDir::new_for_watcher(&self.test_name)?;
		let test_root = test_data.path().to_path_buf();

		// The daemon never walks or watches its own data directory, so it lives
		// beside the test files rather than above them. The harness's own logs
		// go inside it for the same reason: a test observing the index never
		// sees the harness writing.
		let data_dir = test_root.join("data");
		let snapshot_dir = data_dir.join("snapshots");
		tokio::fs::create_dir_all(&snapshot_dir).await?;

		// Initialize tracing
		init_test_tracing(&self.test_name, &snapshot_dir)?;

		// Create config with configurable watcher
		let mut config = TestConfigBuilder::new(data_dir)
			.build()
			.context("Failed to create test config")?;

		// Set watcher state based on builder configuration
		config.services.fs_watcher_enabled = self.watcher_enabled;
		// Enable volume monitoring so VolumeManager detects volumes
		config.services.volume_monitoring_enabled = true;
		config.save()?;

		// Initialize core
		let core = Core::new(config.data_dir.clone())
			.await
			.map_err(|e| anyhow::anyhow!("Failed to initialize core: {}", e))?;

		// Create library
		let library = core
			.libraries
			.create_library(
				format!("{} Library", self.test_name),
				None,
				core.context.clone(),
			)
			.await?;

		// Use the real device UUID so paths resolve to this device
		let device_id = sd_core::device::get_current_device_id();
		// Make device name unique per test to avoid slug collisions in parallel tests
		let device_name = format!("{}-{}", whoami::devicename(), self.test_name);
		register_device(&library, device_id, &device_name).await?;

		// Get device record
		let device_record = entities::device::Entity::find()
			.filter(entities::device::Column::Uuid.eq(device_id))
			.one(library.db().conn())
			.await?
			.ok_or_else(|| anyhow::anyhow!("Device not found after registration"))?;

		// Wrap core in Arc for shared access
		let core = Arc::new(core);

		// Start daemon RPC server if enabled (for TypeScript bridge tests)
		let daemon_socket_addr = if self.daemon_enabled {
			// Find an available port by binding to 0 and getting the actual port
			let temp_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
			let actual_port = temp_listener.local_addr()?.port();
			let socket_addr = format!("127.0.0.1:{}", actual_port);
			drop(temp_listener); // Release the port

			tracing::info!("Starting daemon RPC server on {}", socket_addr);

			let core_for_daemon = core.clone();
			let socket_addr_clone = socket_addr.clone();

			// Spawn daemon server in background
			tokio::spawn(async move {
				let mut server =
					sd_core::infra::daemon::rpc::RpcServer::new(socket_addr_clone, core_for_daemon);
				if let Err(e) = server.start().await {
					tracing::error!("Daemon RPC server error: {}", e);
				}
			});

			// Wait for server to start accepting connections
			tokio::time::sleep(Duration::from_secs(1)).await;

			Some(socket_addr)
		} else {
			None
		};

		Ok(IndexingHarness {
			test_data,
			snapshot_dir,
			core,
			library,
			device_id,
			device_db_id: device_record.id,
			daemon_socket_addr,
		})
	}
}

/// Indexing test harness with convenient helper methods
pub struct IndexingHarness {
	test_data: TestDataDir,
	pub snapshot_dir: PathBuf,
	pub core: Arc<Core>,
	pub library: Arc<sd_core::library::Library>,
	pub device_id: Uuid,
	pub device_db_id: i32,
	daemon_socket_addr: Option<String>,
}

impl IndexingHarness {
	/// Get the temp directory path (for creating test files)
	pub fn temp_path(&self) -> &Path {
		self.test_data.path()
	}

	/// Get access to the snapshot manager (if snapshots enabled via SD_TEST_SNAPSHOTS=1)
	pub fn snapshot_manager(&self) -> Option<&super::SnapshotManager> {
		self.test_data.snapshot_manager()
	}

	/// Get the daemon socket address (only available if daemon is enabled)
	pub fn daemon_socket_addr(&self) -> Option<&str> {
		self.daemon_socket_addr.as_deref()
	}

	/// Walk a directory into the volume index the way a browse does: under
	/// the volume's spelling of the path, into the partition that spelling
	/// selects, clearing whatever an earlier walk left there. The finished walk
	/// marks the directory indexed and, with the watcher enabled, watches it.
	pub async fn index_dir(&self, path: &Path, scope: IndexScope) -> anyhow::Result<()> {
		let context = &self.core.context;
		let cache = context.volume_index();
		let path = match context.volume_manager.locate_path(path).await {
			Some((_, spelled)) => spelled,
			None => path.to_path_buf(),
		};

		let index = cache.create_for_indexing(path.clone());
		cache.clear_for_reindex(&path).await;

		let mut walk = IndexerJob::new(IndexerJobConfig::new(SdPath::local(path), scope, false));
		walk.set_arena(index);
		self.library.jobs().dispatch(walk).await?.wait().await?;
		Ok(())
	}

	/// Create a test directory to write files into
	pub async fn create_test_dir(&self, name: &str) -> anyhow::Result<TestDir> {
		let dir = self.temp_path().join(name);
		tokio::fs::create_dir_all(&dir).await?;

		Ok(TestDir {
			path: dir,
			harness: self,
		})
	}

	/// Shutdown the harness
	pub async fn shutdown(self) -> anyhow::Result<()> {
		let lib_id = self.library.id();

		self.core.libraries.close_library(lib_id).await?;
		drop(self.library);
		self.core
			.shutdown()
			.await
			.map_err(|e| anyhow::anyhow!("Failed to shutdown core: {}", e))?;

		// On Windows, SQLite file locks can persist after shutdown even after WAL checkpoint
		// This is due to the connection pool in SeaORM potentially holding onto connections
		// Give the OS time to release locks to reduce leftover test directories
		// TestDataDir cleanup ignores errors on Windows, so this is just best-effort
		#[cfg(windows)]
		tokio::time::sleep(Duration::from_millis(500)).await;

		// TestDataDir handles cleanup automatically on drop
		Ok(())
	}
}

/// Helper for building test directories with files
pub struct TestDir<'a> {
	path: PathBuf,
	harness: &'a IndexingHarness,
}

impl<'a> TestDir<'a> {
	/// Get the directory path
	pub fn path(&self) -> &Path {
		&self.path
	}

	/// Write a file with content
	pub async fn write_file(&self, relative_path: &str, content: &str) -> anyhow::Result<PathBuf> {
		let file_path = self.path.join(relative_path);

		// Create parent directories if needed
		if let Some(parent) = file_path.parent() {
			tokio::fs::create_dir_all(parent).await?;
		}

		tokio::fs::write(&file_path, content).await?;
		tracing::debug!(path = %file_path.display(), "Created test file");
		Ok(file_path)
	}

	/// Track this directory as a source and wait for its walk. On return the
	/// volume index holds the tree and, with the watcher enabled, watches it.
	pub async fn track(&self) -> anyhow::Result<TrackSourceOutput> {
		let output = TrackSourceAction::from_input(TrackSourceInput {
			path: self.path.clone(),
			name: None,
			overrides: Default::default(),
		})
		.map_err(anyhow::Error::msg)?
		.execute(
			self.harness.library.clone(),
			self.harness.core.context.clone(),
		)
		.await?;

		// A walk that already finished has left the running set, which is the
		// state being waited for
		let job_id = output.job_id.context("tracking dispatched no walk")?;
		if let Some(walk) = self.harness.library.jobs().get_job(JobId(job_id)).await {
			walk.wait().await?;
		}

		Ok(output)
	}

	/// Create a directory
	pub async fn create_dir(&self, relative_path: &str) -> anyhow::Result<PathBuf> {
		let dir_path = self.path.join(relative_path);
		tokio::fs::create_dir_all(&dir_path).await?;
		tracing::debug!(path = %dir_path.display(), "Created test directory");
		Ok(dir_path)
	}

	/// Create files that should be filtered by default rules
	pub async fn create_filtered_files(&self) -> anyhow::Result<()> {
		self.write_file(".DS_Store", "system file").await?;
		self.create_dir("node_modules").await?;
		self.write_file("node_modules/package.json", "{}").await?;
		self.write_file(".git/config", "[core]").await?;
		Ok(())
	}
}
