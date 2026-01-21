//! Integration test utilities for setting up isolated test environments
//!
//! This module provides utilities for creating isolated test environments with:
//! - Custom data directories per test
//! - Structured logging to library/logs
//! - Configurable AppConfig for different test scenarios
//! - Automatic cleanup and resource management
//!
//! ## Example Usage
//!
//! ### Basic Setup
//! ```rust,no_run
//! use sd_core::testing::integration_utils::IntegrationTestSetup;
//!
//! #[tokio::test]
//! async fn my_integration_test() {
//!     // Create test environment with default config
//!     let setup = IntegrationTestSetup::new("my_test").await.unwrap();
//!
//!     // Create core using the test setup's configuration
//!     let core = setup.create_core().await.unwrap();
//!
//!     // Your test logic here...
//!     // The core will use the test setup's custom configuration settings
//!
//!     // Logs are automatically saved to test_data/my_test/library/logs/my_test.log
//!     // Job logs go to test_data/my_test/library/job_logs/
//!
//!     // Cleanup is automatic when setup is dropped
//! }
//! ```
//!
//! ### Custom Configuration
//! ```rust,no_run
//! use sd_core::testing::integration_utils::IntegrationTestSetup;
//!
//! #[tokio::test]
//! async fn test_with_custom_config() {
//!     let setup = IntegrationTestSetup::with_config("custom_test", |builder| {
//!         builder
//!             .log_level("debug")
//!             .networking_enabled(true)
//!             .volume_monitoring_enabled(true)
//!     }).await.unwrap();
//!
//!     let core = setup.create_core().await.unwrap();
//!     // Test with networking and volume monitoring enabled...
//! }
//! ```
//!
//! ### Custom Tracing
//! ```rust,no_run
//! use sd_core::testing::integration_utils::IntegrationTestSetup;
//!
//! #[tokio::test]
//! async fn test_with_debug_logging() {
//!     let setup = IntegrationTestSetup::with_tracing(
//!         "debug_test",
//!         "debug,sd_core=trace,my_module=info"
//!     ).await.unwrap();
//!
//!     // Test with detailed debug logging...
//! }
//! ```

use crate::config::{AppConfig, JobLoggingConfig, Preferences, ServiceConfig};
use std::path::PathBuf;
use std::sync::Once;
use tracing::{info, warn};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Test environment configuration
#[derive(Debug, Clone)]
pub struct TestEnvironment {
	/// Root directory for all test data (e.g., "core/test_data")
	pub test_root: PathBuf,
	/// Specific test data directory (e.g., "core/test_data/test_job_resumption_001")
	pub test_data_dir: PathBuf,
	/// Library data directory within the test (e.g., "core/test_data/test_job_resumption_001/library")
	pub library_data_dir: PathBuf,
	/// Logs directory (e.g., "core/test_data/test_job_resumption_001/library/logs")
	pub logs_dir: PathBuf,
	/// Test name for identification
	pub test_name: String,
}

impl TestEnvironment {
	/// Create a new test environment with the given name
	pub fn new(test_name: impl Into<String>) -> Result<Self, Box<dyn std::error::Error>> {
		let test_name = test_name.into();
		let test_root = PathBuf::from("test_data");
		let test_data_dir = test_root.join(&test_name);
		let library_data_dir = test_data_dir.join("library");
		let logs_dir = library_data_dir.join("logs");

		// Create all necessary directories
		std::fs::create_dir_all(&logs_dir)?;

		Ok(Self {
			test_root,
			test_data_dir,
			library_data_dir,
			logs_dir,
			test_name,
		})
	}

	/// Clean the test environment (remove all data)
	pub fn clean(&self) -> Result<(), Box<dyn std::error::Error>> {
		if self.test_data_dir.exists() {
			std::fs::remove_dir_all(&self.test_data_dir)?;
			info!("Cleaned test environment: {}", self.test_data_dir.display());
		}
		Ok(())
	}

	/// Get the path for a specific log file within this test environment
	pub fn log_file_path(&self, filename: &str) -> PathBuf {
		self.logs_dir.join(filename)
	}

	/// Get the job log path for a specific job ID
	pub fn job_log_path(&self, job_id: uuid::Uuid) -> PathBuf {
		self.library_data_dir
			.join("job_logs")
			.join(format!("{}.log", job_id))
	}
}

/// Test configuration builder for creating custom AppConfigs
#[derive(Debug, Clone)]
pub struct TestConfigBuilder {
	data_dir: PathBuf,
	log_level: String,
	networking_enabled: bool,
	volume_monitoring_enabled: bool,
	fs_watcher_enabled: bool,
	statistics_listener_enabled: bool,
	job_logging_enabled: bool,
	telemetry_enabled: bool,
}

impl TestConfigBuilder {
	/// Create a new test config builder with sensible defaults for testing
	pub fn new(data_dir: PathBuf) -> Self {
		Self {
			data_dir,
			log_level: "warn".to_string(),      // Reduce log noise by default
			networking_enabled: false,          // Disable for faster tests
			volume_monitoring_enabled: false,   // Disable for faster tests
			fs_watcher_enabled: true,           // Usually needed for indexing tests
			statistics_listener_enabled: false, // Disable for faster tests
			job_logging_enabled: true,          // Usually needed for job tests
			telemetry_enabled: false,           // Disable for tests
		}
	}

	/// Set the log level (default: "warn")
	pub fn log_level(mut self, level: impl Into<String>) -> Self {
		self.log_level = level.into();
		self
	}

	/// Enable/disable networking (default: false)
	pub fn networking_enabled(mut self, enabled: bool) -> Self {
		self.networking_enabled = enabled;
		self
	}

	/// Enable/disable volume monitoring (default: false)
	pub fn volume_monitoring_enabled(mut self, enabled: bool) -> Self {
		self.volume_monitoring_enabled = enabled;
		self
	}

	/// Enable/disable filesystem watcher (default: true)
	pub fn fs_watcher_enabled(mut self, enabled: bool) -> Self {
		self.fs_watcher_enabled = enabled;
		self
	}

	/// Enable/disable statistics listener (default: false)
	pub fn statistics_listener_enabled(mut self, enabled: bool) -> Self {
		self.statistics_listener_enabled = enabled;
		self
	}

	/// Enable/disable job logging (default: true)
	pub fn job_logging_enabled(mut self, enabled: bool) -> Self {
		self.job_logging_enabled = enabled;
		self
	}

	/// Enable/disable telemetry (default: false)
	pub fn telemetry_enabled(mut self, enabled: bool) -> Self {
		self.telemetry_enabled = enabled;
		self
	}

	/// Build the AppConfig
	pub fn build(self) -> AppConfig {
		AppConfig {
			version: 4,
			data_dir: self.data_dir,
			log_level: self.log_level,
			telemetry_enabled: self.telemetry_enabled,
			preferences: Preferences::default(),
			job_logging: JobLoggingConfig {
				enabled: self.job_logging_enabled,
				log_directory: "job_logs".to_string(),
				max_file_size: 10 * 1024 * 1024, // 10MB
				include_debug: false,
				log_ephemeral_jobs: false,
			},
			services: ServiceConfig {
				networking_enabled: self.networking_enabled,
				volume_monitoring_enabled: self.volume_monitoring_enabled,
				fs_watcher_enabled: self.fs_watcher_enabled,
				statistics_listener_enabled: self.statistics_listener_enabled,
			},
			logging: crate::config::app_config::LoggingConfig::default(),
		}
	}

	/// Build and save the AppConfig to the data directory
	pub async fn build_and_save(self) -> Result<AppConfig, Box<dyn std::error::Error>> {
		let config = self.build();

		// Ensure the data directory exists
		std::fs::create_dir_all(&config.data_dir)?;

		// Save the config so Core::new() will load our custom settings
		config.save()?;
		info!(
			"Created test configuration at: {} with custom settings",
			config.data_dir.display()
		);
		info!("  - Log level: {}", config.log_level);
		info!(
			"  - Networking enabled: {}",
			config.services.networking_enabled
		);
		info!(
			"  - Volume monitoring enabled: {}",
			config.services.volume_monitoring_enabled
		);
		info!(
			"  - Filesystem watcher enabled: {}",
			config.services.fs_watcher_enabled
		);
		info!("  - Job logging enabled: {}", config.job_logging.enabled);

		Ok(config)
	}
}

/// Initialize structured logging for integration tests
pub fn initialize_test_tracing(
	test_env: &TestEnvironment,
	rust_log_override: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
	static INIT: Once = Once::new();
	let mut result: Result<(), Box<dyn std::error::Error>> = Ok(());

	INIT.call_once(|| {
		// Set up environment filter with detailed logging for tests
		let env_filter = rust_log_override
			.map(|s| s.to_string())
			.or_else(|| std::env::var("RUST_LOG").ok())
			.unwrap_or_else(|| {
				format!(
					"warn,sd_core=info,{}=info,iroh::magicsock::transports::relay=error",
					test_env.test_name
				)
			});

		// Create file appender that rotates daily in the test's log directory
		let file_appender = RollingFileAppender::new(
			Rotation::DAILY,
			&test_env.logs_dir,
			format!("{}.log", test_env.test_name),
		);

		// Set up layered subscriber with stdout and file output
		if let Err(e) = tracing_subscriber::registry()
			.with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(env_filter)))
			.with(
				fmt::layer()
					.with_target(true)
					.with_thread_ids(true)
					.with_line_number(true)
					.with_writer(std::io::stdout),
			)
			.with(
				fmt::layer()
					.with_target(true)
					.with_thread_ids(true)
					.with_line_number(true)
					.with_ansi(false) // No ANSI colors in log files
					.with_writer(file_appender),
			)
			.try_init()
		{
			result = Err(format!("Failed to initialize tracing: {}", e).into());
		}
	});

	result
}

/// Complete test setup utility that combines environment, config, and tracing
pub struct IntegrationTestSetup {
	pub environment: TestEnvironment,
	pub config: AppConfig,
}

impl IntegrationTestSetup {
	/// Create a new integration test setup with default configuration
	pub async fn new(test_name: impl Into<String>) -> Result<Self, Box<dyn std::error::Error>> {
		let environment = TestEnvironment::new(test_name)?;

		// Clean any existing data
		environment.clean()?;

		// Recreate directories
		std::fs::create_dir_all(&environment.logs_dir)?;

		// Initialize tracing
		initialize_test_tracing(&environment, None)?;

		// Create default config
		let config = TestConfigBuilder::new(environment.library_data_dir.clone())
			.build_and_save()
			.await?;

		Ok(Self {
			environment,
			config,
		})
	}

	/// Create a new integration test setup with custom configuration
	pub async fn with_config<F>(
		test_name: impl Into<String>,
		config_builder: F,
	) -> Result<Self, Box<dyn std::error::Error>>
	where
		F: FnOnce(TestConfigBuilder) -> TestConfigBuilder,
	{
		let environment = TestEnvironment::new(test_name)?;

		// Clean any existing data
		environment.clean()?;

		// Recreate directories
		std::fs::create_dir_all(&environment.logs_dir)?;

		// Initialize tracing
		initialize_test_tracing(&environment, None)?;

		// Create custom config
		let builder = TestConfigBuilder::new(environment.library_data_dir.clone());
		let config = config_builder(builder).build_and_save().await?;

		Ok(Self {
			environment,
			config,
		})
	}

	/// Create a new integration test setup with custom tracing
	pub async fn with_tracing(
		test_name: impl Into<String>,
		rust_log_override: &str,
	) -> Result<Self, Box<dyn std::error::Error>> {
		let environment = TestEnvironment::new(test_name)?;

		// Clean any existing data
		environment.clean()?;

		// Recreate directories
		std::fs::create_dir_all(&environment.logs_dir)?;

		// Initialize custom tracing
		initialize_test_tracing(&environment, Some(rust_log_override))?;

		// Create default config
		let config = TestConfigBuilder::new(environment.library_data_dir.clone())
			.build_and_save()
			.await?;

		Ok(Self {
			environment,
			config,
		})
	}

	/// Get the data directory for core initialization
	pub fn data_dir(&self) -> &PathBuf {
		&self.config.data_dir
	}

	/// Get a reference to the test environment
	pub fn env(&self) -> &TestEnvironment {
		&self.environment
	}

	/// Create a Core instance using the test setup's configuration
	///
	/// This method ensures that the custom AppConfig settings from the test setup
	/// are properly applied when initializing the Core.
	pub async fn create_core(&self) -> Result<crate::Core, Box<dyn std::error::Error>> {
		info!(
			"Creating Core with test configuration from: {}",
			self.data_dir().display()
		);

		// Core::new() will load our saved AppConfig from disk
		let core = crate::Core::new(self.data_dir().clone()).await?;

		// Verify our config was loaded correctly
		{
			let config_arc = core.config();
			let loaded_config = config_arc.read().await;
			info!("Core initialized with config:");
			info!("  - Log level: {}", loaded_config.log_level);
			info!(
				"  - Networking enabled: {}",
				loaded_config.services.networking_enabled
			);
			info!(
				"  - Volume monitoring enabled: {}",
				loaded_config.services.volume_monitoring_enabled
			);
			info!(
				"  - Filesystem watcher enabled: {}",
				loaded_config.services.fs_watcher_enabled
			);
			info!(
				"  - Job logging enabled: {}",
				loaded_config.job_logging.enabled
			);
		}

		Ok(core)
	}

	/// Clean up the test environment
	pub fn cleanup(self) -> Result<(), Box<dyn std::error::Error>> {
		self.environment.clean()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn test_environment_creation() {
		let env = TestEnvironment::new("test_example").unwrap();

		assert!(env.test_data_dir.ends_with("test_data/test_example"));
		assert!(env
			.library_data_dir
			.ends_with("test_data/test_example/library"));
		assert!(env
			.logs_dir
			.ends_with("test_data/test_example/library/logs"));
		assert!(env.logs_dir.exists());

		// Cleanup
		env.clean().unwrap();
	}

	#[tokio::test]
	async fn test_config_builder() {
		let temp_dir = std::env::temp_dir().join("test_config_builder");
		std::fs::create_dir_all(&temp_dir).unwrap();

		let config = TestConfigBuilder::new(temp_dir.clone())
			.log_level("debug")
			.networking_enabled(true)
			.build();

		assert_eq!(config.log_level, "debug");
		assert_eq!(config.services.networking_enabled, true);
		assert_eq!(config.services.volume_monitoring_enabled, false); // default

		// Cleanup
		std::fs::remove_dir_all(&temp_dir).ok();
	}

	#[tokio::test]
	async fn test_integration_setup() {
		let setup = IntegrationTestSetup::new("test_integration_setup")
			.await
			.unwrap();

		assert!(setup.data_dir().exists());
		assert!(setup.env().logs_dir.exists());

		// Cleanup
		setup.cleanup().unwrap();
	}
}
