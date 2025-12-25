//! Indexing test harness and utilities
//!
//! Provides reusable components for indexing integration tests,
//! reducing boilerplate and making it easy to test change detection.

use super::{init_test_tracing, register_device, wait_for_indexing, TestConfigBuilder};
use anyhow::Context;
use sd_core::{
	infra::db::entities::{self, entry_closure},
	location::{create_location, IndexMode, LocationCreateArgs},
	Core,
};
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
use std::{
	path::{Path, PathBuf},
	sync::Arc,
};
use tempfile::TempDir;
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
		// Use home directory for proper filesystem watcher support on macOS
		let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
		let test_root = PathBuf::from(home).join(format!(".spacedrive_test_{}", self.test_name));

		// Clean up any existing test directory
		let _ = tokio::fs::remove_dir_all(&test_root).await;
		tokio::fs::create_dir_all(&test_root).await?;

		let snapshot_dir = test_root.join("snapshots");
		tokio::fs::create_dir_all(&snapshot_dir).await?;

		// Initialize tracing
		init_test_tracing(&self.test_name, &snapshot_dir)?;

		// Create config with configurable watcher
		let mut config = TestConfigBuilder::new(test_root.clone())
			.build()
			.context("Failed to create test config")?;

		// Set watcher state based on builder configuration
		config.services.fs_watcher_enabled = self.watcher_enabled;
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

		// Use the real device UUID so the watcher can find locations
		let device_id = sd_core::device::get_current_device_id();
		let device_name = whoami::devicename();
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
			_test_name: self.test_name,
			_test_root: test_root,
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
	_test_name: String,
	_test_root: PathBuf,
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
		&self._test_root
	}

	/// Get the daemon socket address (only available if daemon is enabled)
	pub fn daemon_socket_addr(&self) -> Option<&str> {
		self.daemon_socket_addr.as_deref()
	}

	/// Create a test location directory with files
	pub async fn create_test_location(&self, name: &str) -> anyhow::Result<TestLocation> {
		let location_dir = self.temp_path().join(name);
		tokio::fs::create_dir_all(&location_dir).await?;

		Ok(TestLocation {
			path: location_dir,
			harness: self,
		})
	}

	/// Add a location and wait for indexing to complete
	pub async fn add_and_index_location(
		&self,
		path: impl AsRef<Path>,
		name: &str,
		mode: IndexMode,
	) -> anyhow::Result<LocationHandle> {
		let path = path.as_ref();

		tracing::info!(
			path = %path.display(),
			name = %name,
			mode = ?mode,
			"Creating and indexing location"
		);

		let location_args = LocationCreateArgs {
			path: path.to_path_buf(),
			name: Some(name.to_string()),
			index_mode: mode,
		};

		let location_db_id = create_location(
			self.library.clone(),
			&self.core.events,
			location_args,
			self.device_db_id,
		)
		.await?;

		// Get the location record to find its entry_id
		let location_record = entities::location::Entity::find_by_id(location_db_id)
			.one(self.library.db().conn())
			.await?
			.ok_or_else(|| anyhow::anyhow!("Location not found after creation"))?;

		// Wait for indexing to complete
		wait_for_indexing(&self.library, location_db_id, Duration::from_secs(30)).await?;

		// Register location with watcher so it can detect changes
		if let Some(watcher) = self.core.context.get_fs_watcher().await {
			use sd_core::ops::indexing::{handlers::LocationMeta, rules::RuleToggles};

			let lib_cfg = self.library.config().await;
			let idx_cfg = lib_cfg.settings.indexer;

			let location_meta = LocationMeta {
				id: location_record.uuid,
				library_id: self.library.id(),
				root_path: path.to_path_buf(),
				rule_toggles: RuleToggles {
					no_system_files: idx_cfg.no_system_files,
					no_hidden: idx_cfg.no_hidden,
					no_git: idx_cfg.no_git,
					gitignore: idx_cfg.gitignore,
					only_images: idx_cfg.only_images,
					no_dev_dirs: idx_cfg.no_dev_dirs,
				},
			};

			watcher.watch_location(location_meta).await?;
			tracing::info!(
				location_uuid = %location_record.uuid,
				"Registered location with watcher"
			);
		} else {
			tracing::warn!("Watcher not available, location changes will not be detected");
		}

		tracing::info!(
			location_id = location_db_id,
			"Location indexed successfully"
		);

		Ok(LocationHandle {
			db_id: location_db_id,
			uuid: location_record.uuid,
			entry_id: location_record.entry_id,
			path: path.to_path_buf(),
			harness: self,
		})
	}

	/// Shutdown the harness
	pub async fn shutdown(self) -> anyhow::Result<()> {
		let lib_id = self.library.id();
		let test_root = self._test_root.clone();

		self.core.libraries.close_library(lib_id).await?;
		drop(self.library);
		self.core
			.shutdown()
			.await
			.map_err(|e| anyhow::anyhow!("Failed to shutdown core: {}", e))?;

		// Clean up test directory
		tokio::fs::remove_dir_all(&test_root).await?;

		Ok(())
	}
}

/// Helper for building test locations with files
pub struct TestLocation<'a> {
	path: PathBuf,
	harness: &'a IndexingHarness,
}

impl<'a> TestLocation<'a> {
	/// Get the location path
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

	/// Index this location with the specified mode
	pub async fn index(&self, name: &str, mode: IndexMode) -> anyhow::Result<LocationHandle<'a>> {
		self.harness
			.add_and_index_location(&self.path, name, mode)
			.await
	}
}

/// Handle to an indexed location with helper methods
pub struct LocationHandle<'a> {
	pub db_id: i32,
	pub uuid: Uuid,
	pub entry_id: Option<i32>,
	pub path: PathBuf,
	harness: &'a IndexingHarness,
}

impl<'a> LocationHandle<'a> {
	/// Get all entry IDs under this location (including the root)
	pub async fn get_all_entry_ids(&self) -> anyhow::Result<Vec<i32>> {
		let location_id = self
			.entry_id
			.ok_or_else(|| anyhow::anyhow!("Location has no entry_id"))?;

		let descendant_ids: Vec<i32> = entry_closure::Entity::find()
			.filter(entry_closure::Column::AncestorId.eq(location_id))
			.all(self.harness.library.db().conn())
			.await?
			.into_iter()
			.map(|ec| ec.descendant_id)
			.collect();

		let mut all_ids = vec![location_id];
		all_ids.extend(descendant_ids);
		Ok(all_ids)
	}

	/// Count total entries under this location
	pub async fn count_entries(&self) -> anyhow::Result<u64> {
		let entry_ids = self.get_all_entry_ids().await?;
		Ok(entry_ids.len() as u64)
	}

	/// Count files under this location
	pub async fn count_files(&self) -> anyhow::Result<u64> {
		let entry_ids = self.get_all_entry_ids().await?;
		let count = entities::entry::Entity::find()
			.filter(entities::entry::Column::Id.is_in(entry_ids))
			.filter(entities::entry::Column::Kind.eq(0)) // Files
			.count(self.harness.library.db().conn())
			.await?;
		Ok(count)
	}

	/// Count directories under this location
	pub async fn count_directories(&self) -> anyhow::Result<u64> {
		let entry_ids = self.get_all_entry_ids().await?;
		let count = entities::entry::Entity::find()
			.filter(entities::entry::Column::Id.is_in(entry_ids))
			.filter(entities::entry::Column::Kind.eq(1)) // Directories
			.count(self.harness.library.db().conn())
			.await?;
		Ok(count)
	}

	/// Get all entries under this location
	pub async fn get_all_entries(&self) -> anyhow::Result<Vec<entities::entry::Model>> {
		let entry_ids = self.get_all_entry_ids().await?;
		let entries = entities::entry::Entity::find()
			.filter(entities::entry::Column::Id.is_in(entry_ids))
			.all(self.harness.library.db().conn())
			.await?;
		Ok(entries)
	}

	/// Verify that no filtered files/directories are indexed
	pub async fn verify_no_filtered_entries(&self) -> anyhow::Result<()> {
		let entries = self.get_all_entries().await?;

		for entry in &entries {
			anyhow::ensure!(
				entry.name != ".DS_Store",
				"System file .DS_Store should be filtered"
			);
			anyhow::ensure!(
				entry.name != "node_modules",
				"Dev directory node_modules should be filtered"
			);
			anyhow::ensure!(entry.name != ".git", "Git directory should be filtered");
		}

		Ok(())
	}

	/// Verify entries with inodes
	pub async fn verify_inode_tracking(&self) -> anyhow::Result<()> {
		let entry_ids = self.get_all_entry_ids().await?;
		let entries_with_inodes = entities::entry::Entity::find()
			.filter(entities::entry::Column::Id.is_in(entry_ids))
			.filter(entities::entry::Column::Inode.is_not_null())
			.count(self.harness.library.db().conn())
			.await?;

		anyhow::ensure!(
			entries_with_inodes > 0,
			"At least some entries should have inode tracking"
		);

		Ok(())
	}

	/// Write a new file to the location
	pub async fn write_file(&self, relative_path: &str, content: &str) -> anyhow::Result<PathBuf> {
		let file_path = self.path.join(relative_path);

		if let Some(parent) = file_path.parent() {
			tokio::fs::create_dir_all(parent).await?;
		}

		tokio::fs::write(&file_path, content).await?;
		tracing::debug!(path = %file_path.display(), "Wrote file to indexed location");
		Ok(file_path)
	}

	/// Modify an existing file
	pub async fn modify_file(&self, relative_path: &str, new_content: &str) -> anyhow::Result<()> {
		let file_path = self.path.join(relative_path);
		tokio::fs::write(&file_path, new_content)
			.await
			.context("Failed to modify file")?;
		tracing::debug!(path = %file_path.display(), "Modified file");
		Ok(())
	}

	/// Delete a file
	pub async fn delete_file(&self, relative_path: &str) -> anyhow::Result<()> {
		let file_path = self.path.join(relative_path);
		tokio::fs::remove_file(&file_path)
			.await
			.context("Failed to delete file")?;
		tracing::debug!(path = %file_path.display(), "Deleted file");
		Ok(())
	}

	/// Move/rename a file
	pub async fn move_file(&self, from: &str, to: &str) -> anyhow::Result<()> {
		let from_path = self.path.join(from);
		let to_path = self.path.join(to);

		if let Some(parent) = to_path.parent() {
			tokio::fs::create_dir_all(parent).await?;
		}

		tokio::fs::rename(&from_path, &to_path)
			.await
			.context("Failed to move file")?;
		tracing::debug!(
			from = %from_path.display(),
			to = %to_path.display(),
			"Moved file"
		);
		Ok(())
	}

	/// Re-index this location and wait for completion
	pub async fn reindex(&self) -> anyhow::Result<()> {
		use sd_core::{
			domain::addressing::SdPath,
			ops::indexing::{IndexerJob, IndexerJobConfig},
		};

		tracing::info!(
			location_uuid = %self.uuid,
			"Re-indexing location"
		);

		// Get the current index mode from the location
		let location_record = entities::location::Entity::find_by_id(self.db_id)
			.one(self.harness.library.db().conn())
			.await?
			.ok_or_else(|| anyhow::anyhow!("Location not found"))?;

		let index_mode = match location_record.index_mode.as_str() {
			"shallow" => sd_core::domain::IndexMode::Shallow,
			"content" => sd_core::domain::IndexMode::Content,
			"deep" => sd_core::domain::IndexMode::Deep,
			_ => sd_core::domain::IndexMode::Content,
		};

		// Create and dispatch indexer job
		let config = IndexerJobConfig::new(self.uuid, SdPath::local(&self.path), index_mode);
		let job = IndexerJob::new(config);

		let handle = self.harness.library.jobs().dispatch(job).await?;

		// Wait for re-indexing job to complete using the handle's wait method
		handle.wait().await?;

		tracing::info!("Re-indexing completed");
		Ok(())
	}

	/// Verify closure table integrity
	///
	/// This is critical for folder renames! When a folder is renamed, all children
	/// must remain properly connected via the closure table. Without this, queries
	/// that traverse the hierarchy will miss entries.
	pub async fn verify_closure_table_integrity(&self) -> anyhow::Result<()> {
		use sea_orm::sea_query::Expr;
		use std::collections::HashSet;

		let db = self.harness.library.db().conn();
		let location_id = self
			.entry_id
			.ok_or_else(|| anyhow::anyhow!("Location has no entry_id"))?;

		// Get all entries that should be in the location (via parent_id traversal)
		let mut all_entries_via_parent = HashSet::new();
		let mut queue = vec![location_id];

		while let Some(parent_id) = queue.pop() {
			all_entries_via_parent.insert(parent_id);

			let children = entities::entry::Entity::find()
				.filter(entities::entry::Column::ParentId.eq(parent_id))
				.all(db)
				.await?;

			for child in children {
				queue.push(child.id);
			}
		}

		// Get all entries via closure table
		let entries_via_closure: HashSet<i32> = entry_closure::Entity::find()
			.filter(entry_closure::Column::AncestorId.eq(location_id))
			.all(db)
			.await?
			.into_iter()
			.map(|ec| ec.descendant_id)
			.collect();

		// The closure table should contain ALL entries found via parent traversal
		let missing_from_closure: Vec<_> = all_entries_via_parent
			.difference(&entries_via_closure)
			.collect();

		if !missing_from_closure.is_empty() {
			// Get details about the missing entries for better error messages
			let missing_entries = entities::entry::Entity::find()
				.filter(
					entities::entry::Column::Id
						.is_in(missing_from_closure.iter().copied().copied()),
				)
				.all(db)
				.await?;

			let mut error_msg = format!(
				"❌ Closure table is corrupted! {} entries are missing from closure table but exist via parent_id:\n",
				missing_from_closure.len()
			);

			for entry in missing_entries.iter().take(10) {
				error_msg.push_str(&format!(
					"  - Entry {} (name: '{}', kind: {})\n",
					entry.id, entry.name, entry.kind
				));
			}

			if missing_entries.len() > 10 {
				error_msg.push_str(&format!("  ... and {} more\n", missing_entries.len() - 10));
			}

			anyhow::bail!(error_msg);
		}

		tracing::debug!(
			"✅ Closure table integrity verified: {} entries properly connected",
			all_entries_via_parent.len()
		);

		Ok(())
	}
}
