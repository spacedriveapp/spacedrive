//! Application configuration

use super::{default_data_dir, Preferences};
use crate::config::migration::Migrate;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tracing::{info, warn};

/// Main application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
	/// Config schema version
	pub version: u32,

	/// Data directory path
	pub data_dir: PathBuf,

	/// Logging level
	pub log_level: String,

	/// Whether telemetry is enabled
	pub telemetry_enabled: bool,

	/// User preferences
	pub preferences: Preferences,

	/// Job logging configuration
	#[serde(default)]
	pub job_logging: JobLoggingConfig,

	/// Service configuration
	#[serde(default)]
	pub services: ServiceConfig,

	/// Daemon logging configuration with multi-stream support
	#[serde(default)]
	pub logging: LoggingConfig,
}

/// Configuration for core services
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
	/// Whether networking is enabled
	pub networking_enabled: bool,

	/// Whether volume monitoring is enabled
	pub volume_monitoring_enabled: bool,

	/// Whether filesystem watcher is enabled
	pub fs_watcher_enabled: bool,

	/// Whether statistics listener is enabled
	#[serde(default = "default_true")]
	pub statistics_listener_enabled: bool,
}

fn default_true() -> bool {
	true
}

impl Default for ServiceConfig {
	fn default() -> Self {
		Self {
			networking_enabled: true,
			volume_monitoring_enabled: true,
			fs_watcher_enabled: true,
			statistics_listener_enabled: true,
		}
	}
}

/// Configuration for job-specific logging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobLoggingConfig {
	/// Whether job logging is enabled
	pub enabled: bool,

	/// Directory for job logs (relative to data_dir)
	pub log_directory: String,

	/// Maximum log file size in bytes (0 = unlimited)
	pub max_file_size: u64,

	/// Whether to include debug logs
	pub include_debug: bool,

	/// Whether to create log files for ephemeral (non-persistent) jobs
	#[serde(default)]
	pub log_ephemeral_jobs: bool,
}

impl Default for JobLoggingConfig {
	fn default() -> Self {
		Self {
			enabled: true,
			log_directory: "job_logs".to_string(),
			max_file_size: 10 * 1024 * 1024, // 10MB default
			include_debug: false,
			log_ephemeral_jobs: false,
		}
	}
}

/// Configuration for a single log stream
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogStreamConfig {
	/// Name of the log stream
	pub name: String,

	/// Output file name (relative to logs directory)
	pub file_name: String,

	/// RUST_LOG-style filter for this stream
	/// Examples:
	/// - "sd_core::service::watcher=debug" - all watcher logs at debug+
	/// - "sd_core::infra::event=trace" - event system at trace level
	/// - "sd_core::service::watcher=debug,sd_core::service::watcher::platform=trace" - multiple targets
	pub filter: String,

	/// Whether this stream is enabled
	pub enabled: bool,
}

/// Configuration for daemon-level logging with multiple streams
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
	/// Main daemon log filter (used for daemon.log and stdout)
	pub main_filter: String,

	/// Additional log streams with custom filters
	#[serde(default)]
	pub streams: Vec<LogStreamConfig>,
}

impl Default for LoggingConfig {
	fn default() -> Self {
		Self {
			main_filter: "sd_core=info,spacedrive=info".to_string(),
			streams: vec![],
		}
	}
}

impl AppConfig {
	/// Load configuration from the default location
	pub fn load() -> Result<Self> {
		let data_dir = default_data_dir()?;
		Self::load_from(&data_dir)
	}

	/// Load configuration from a specific data directory
	pub fn load_from(data_dir: &PathBuf) -> Result<Self> {
		let config_path = data_dir.join("spacedrive.json");

		if config_path.exists() {
			info!("Loading config from {:?}", config_path);
			let json = fs::read_to_string(&config_path)?;
			let mut config: AppConfig = serde_json::from_str(&json)?;

			// Update data_dir if it doesn't match the current directory
			// This handles iOS container changes during development
			if config.data_dir != *data_dir {
				info!(
					"Updating config data_dir from {:?} to {:?}",
					config.data_dir, data_dir
				);
				config.data_dir = data_dir.clone();
				config.save()?;
			}

			// Apply migrations if needed
			if config.version < Self::target_version() {
				info!(
					"Migrating config from v{} to v{}",
					config.version,
					Self::target_version()
				);
				config.migrate()?;
				config.save()?;
			}

			Ok(config)
		} else {
			warn!("No config found, creating default at {:?}", config_path);
			let config = Self::default_with_dir(data_dir.clone());
			config.save()?;
			Ok(config)
		}
	}

	/// Load or create configuration
	pub fn load_or_create(data_dir: &PathBuf) -> Result<Self> {
		Self::load_from(data_dir).or_else(|_| {
			let config = Self::default_with_dir(data_dir.clone());
			config.save()?;
			Ok(config)
		})
	}

	/// Create default configuration with specific data directory
	pub fn default_with_dir(data_dir: PathBuf) -> Self {
		Self {
			version: Self::target_version(),
			data_dir,
			log_level: "info".to_string(),
			telemetry_enabled: true,
			preferences: Preferences::default(),
			job_logging: JobLoggingConfig::default(),
			services: ServiceConfig::default(),
			logging: LoggingConfig::default(),
		}
	}

	/// Save configuration to disk
	pub fn save(&self) -> Result<()> {
		// Ensure directory exists
		fs::create_dir_all(&self.data_dir)?;

		let config_path = self.data_dir.join("spacedrive.json");
		let json = serde_json::to_string_pretty(self)?;
		fs::write(&config_path, json)?;
		info!("Saved config to {:?}", config_path);
		Ok(())
	}

	/// Get the path for logs directory
	pub fn logs_dir(&self) -> PathBuf {
		self.data_dir.join("logs")
	}

	/// Get the path for libraries directory
	pub fn libraries_dir(&self) -> PathBuf {
		self.data_dir.join("libraries")
	}

	/// Ensure all required directories exist
	///
	/// Note: On iOS, create_dir_all() can fail with "Operation not permitted" due to sandboxing
	/// restrictions, even on new directories. We skip directory creation on iOS since directories
	/// are created properly during first run and maintained by the system.
	pub fn ensure_directories(&self) -> Result<()> {
		#[cfg(target_os = "ios")]
		{
			// On iOS, skip directory creation to avoid sandboxing permission errors
			// Directories are created during first run and maintained by iOS
			return Ok(());
		}

		#[cfg(not(target_os = "ios"))]
		{
			fs::create_dir_all(&self.data_dir)?;
			fs::create_dir_all(self.logs_dir())?;
			fs::create_dir_all(self.libraries_dir())?;
			// Job logs are now stored per-library
		}

		Ok(())
	}
}

impl Default for AppConfig {
	fn default() -> Self {
		let data_dir = default_data_dir().unwrap_or_else(|_| PathBuf::from("."));
		Self::default_with_dir(data_dir)
	}
}

impl Migrate for AppConfig {
	fn current_version(&self) -> u32 {
		self.version
	}

	fn target_version() -> u32 {
		4 // Updated schema version for multi-stream logging
	}

	fn migrate(&mut self) -> Result<()> {
		match self.version {
			0 => {
				// Future migration from v0 to v1 would go here
				self.version = 1;
				self.migrate() // Continue migration chain
			}
			1 => {
				// Migration from v1 to v2: Add job logging config
				self.job_logging = JobLoggingConfig::default();
				self.version = 2;
				self.migrate()
			}
			2 => {
				// Migration from v2 to v3: Add service configuration and remove P2P config
				self.services = ServiceConfig::default();
				self.version = 3;
				self.migrate()
			}
			3 => {
				// Migration from v3 to v4: Add multi-stream logging configuration
				self.logging = LoggingConfig::default();
				self.version = 4;
				Ok(())
			}
			4 => Ok(()), // Already at target version
			v => Err(anyhow!("Unknown config version: {}", v)),
		}
	}
}
