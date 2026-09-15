//! Application configuration management

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub mod app_config;
pub mod migration;

pub use app_config::{
	AppConfig, JobLoggingConfig, LogStreamConfig, LoggingConfig, ProxyPairingConfig, ServiceConfig,
	SpacebotConfig,
};
pub use migration::Migrate;

/// Default data directory: `~/.spacedrive` on desktop, platform data dir on mobile.
pub fn default_data_dir() -> Result<PathBuf> {
	#[cfg(not(any(target_os = "ios", target_os = "android")))]
	let dir = dirs::home_dir()
		.ok_or_else(|| anyhow!("Could not determine home directory"))?
		.join(".spacedrive");

	#[cfg(target_os = "ios")]
	let dir = dirs::data_dir()
		.ok_or_else(|| anyhow!("Could not determine data directory"))?
		.join("spacedrive");

	#[cfg(target_os = "android")]
	let dir = dirs::data_dir()
		.ok_or_else(|| anyhow!("Could not determine data directory"))?
		.join("spacedrive");

	// Create directory if it doesn't exist
	fs::create_dir_all(&dir)?;

	Ok(dir)
}

static OWN_DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Record where this process keeps its own data, so walks and watchers can
/// refuse to index it. A daemon whose data dir sits on an indexed drive
/// otherwise records its own SQLite journals, which are born and deleted
/// fast enough to give one path two identities and poison the batch around
/// it. First caller wins; a process has one data dir.
pub fn mark_own_data_dir(dir: &Path) {
	let canonical = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
	let _ = OWN_DATA_DIR.set(canonical);
}

/// Whether a path is inside this process's own data directory.
pub fn is_own_data(path: &Path) -> bool {
	OWN_DATA_DIR.get().is_some_and(|own| path.starts_with(own))
}

/// User preferences
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preferences {
	pub theme: String,    // "light", "dark", "system"
	pub language: String, // ISO 639-1 code
}

impl Default for Preferences {
	fn default() -> Self {
		Self {
			theme: "system".to_string(),
			language: "en".to_string(),
		}
	}
}
