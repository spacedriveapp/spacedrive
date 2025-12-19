//! Simple File Sync Test
//!
//! This tests the file sync service initialization and basic operations
//! without requiring full database setup or actual file indexing.

use sd_core::{infra::db::entities::sync_conduit, Core};
use std::sync::Arc;
use tempfile::TempDir;

/// Test setup with a core and library
struct FileSyncTestSetup {
	_temp_dir: TempDir,
	core: Core,
	library: Arc<sd_core::library::Library>,
}

impl FileSyncTestSetup {
	/// Create a new test setup
	async fn new() -> anyhow::Result<Self> {
		let _ = tracing_subscriber::fmt()
			.with_env_filter("sd_core=info")
			.with_test_writer()
			.try_init();

		let temp_dir = TempDir::new()?;

		let config = sd_core::config::AppConfig {
			version: 3,
			data_dir: temp_dir.path().to_path_buf(),
			log_level: "info".to_string(),
			telemetry_enabled: false,
			preferences: sd_core::config::Preferences::default(),
			job_logging: sd_core::config::JobLoggingConfig::default(),
			services: sd_core::config::ServiceConfig {
				networking_enabled: false,
				volume_monitoring_enabled: false,
				fs_watcher_enabled: false,
			},
			logging: sd_core::config::LoggingConfig::default(),
		};
		config.save()?;

		let core = Core::new(temp_dir.path().to_path_buf())
			.await
			.map_err(|e| anyhow::anyhow!("{}", e))?;

		let library = core
			.libraries
			.create_library("File Sync Test Library", None, core.context.clone())
			.await?;

		// Initialize file sync service
		library.init_file_sync_service()?;

		Ok(Self {
			_temp_dir: temp_dir,
			core,
			library,
		})
	}
}

#[tokio::test]
async fn test_file_sync_service_initialization() {
	let setup = FileSyncTestSetup::new().await.unwrap();

	// Verify file sync service was initialized
	assert!(setup.library.file_sync_service().is_some());

	println!("✓ File sync service initialized successfully");
}

#[tokio::test]
async fn test_file_sync_service_structure() {
	let setup = FileSyncTestSetup::new().await.unwrap();

	let file_sync = setup.library.file_sync_service().unwrap();

	// Verify we can access the conduit manager
	let conduit_manager = file_sync.conduit_manager();
	assert!(Arc::strong_count(conduit_manager) > 0);

	println!("✓ File sync service has proper structure");
	println!("  - ConduitManager accessible");
}

#[tokio::test]
async fn test_sync_modes() {
	// Test that all sync modes can be converted properly
	let mirror = sync_conduit::SyncMode::Mirror;
	assert_eq!(mirror.as_str(), "mirror");
	assert_eq!(
		sync_conduit::SyncMode::from_str("mirror"),
		Some(sync_conduit::SyncMode::Mirror)
	);

	let bidirectional = sync_conduit::SyncMode::Bidirectional;
	assert_eq!(bidirectional.as_str(), "bidirectional");
	assert_eq!(
		sync_conduit::SyncMode::from_str("bidirectional"),
		Some(sync_conduit::SyncMode::Bidirectional)
	);

	let selective = sync_conduit::SyncMode::Selective;
	assert_eq!(selective.as_str(), "selective");
	assert_eq!(
		sync_conduit::SyncMode::from_str("selective"),
		Some(sync_conduit::SyncMode::Selective)
	);

	// Invalid mode
	assert_eq!(sync_conduit::SyncMode::from_str("invalid"), None);

	println!("✓ Sync modes working correctly");
	println!("  - Mirror: {}", mirror);
	println!("  - Bidirectional: {}", bidirectional);
	println!("  - Selective: {}", selective);
}

#[tokio::test]
async fn test_multiple_libraries_with_file_sync() {
	let temp_dir = TempDir::new().unwrap();
	let core = Core::new(temp_dir.path().to_path_buf()).await.unwrap();

	// Create multiple libraries
	let lib1 = core
		.libraries
		.create_library("Library 1", None, core.context.clone())
		.await
		.unwrap();
	lib1.init_file_sync_service().unwrap();

	let lib2 = core
		.libraries
		.create_library("Library 2", None, core.context.clone())
		.await
		.unwrap();
	lib2.init_file_sync_service().unwrap();

	// Both should have file sync services
	assert!(lib1.file_sync_service().is_some());
	assert!(lib2.file_sync_service().is_some());

	println!("✓ Multiple libraries can have file sync services");
	println!("  - Library 1: {}", lib1.name().await);
	println!("  - Library 2: {}", lib2.name().await);
}

#[tokio::test]
async fn test_file_sync_service_idempotent_initialization() {
	let temp_dir = TempDir::new().unwrap();
	let core = Core::new(temp_dir.path().to_path_buf()).await.unwrap();

	// Create library
	let library = core
		.libraries
		.create_library("Test Library", None, core.context.clone())
		.await
		.unwrap();

	// Initialize file sync
	library.init_file_sync_service().unwrap();
	assert!(library.file_sync_service().is_some());

	// Trying to initialize again should be idempotent (warning but no error)
	library.init_file_sync_service().unwrap();
	assert!(library.file_sync_service().is_some());

	println!("✓ File sync service initialization is idempotent");
	println!("  - First initialization: OK");
	println!("  - Second initialization: OK (warning logged)");
}
