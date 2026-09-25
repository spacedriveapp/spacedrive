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

static OWN_DATA_DIRS: OnceLock<Vec<PathBuf>> = OnceLock::new();

/// Record where this process keeps its own data, so walks and watchers can
/// refuse to index it. A daemon whose data dir sits on an indexed drive
/// otherwise records its own SQLite journals, which are born and deleted
/// fast enough to give one path two identities and poison the batch around
/// it. First caller wins; a process has one data dir.
///
/// Every macOS firmlink spelling of the directory is held, because watcher
/// events arrive under `/System/Volumes/Data` while the configured path is
/// usually the `/Users` alias; `canonicalize` resolves symlinks but not
/// firmlinks, so a single spelling misses half the events.
pub fn mark_own_data_dir(dir: &Path) {
	let _ = OWN_DATA_DIRS.set(path_spellings(dir));
}

/// Every spelling a directory can appear under in an event, a walk, or the
/// mount table.
pub(crate) fn path_spellings(dir: &Path) -> Vec<PathBuf> {
	let canonical = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
	let mut spellings = vec![canonical];
	if dir != spellings[0] {
		spellings.push(dir.to_path_buf());
	}

	#[cfg(target_os = "macos")]
	{
		const DATA_VOLUME: &str = "/System/Volumes/Data";
		for spelling in spellings.clone() {
			if let Ok(rest) = spelling.strip_prefix(DATA_VOLUME) {
				spellings.push(Path::new("/").join(rest));
			} else if let Ok(rest) = spelling.strip_prefix("/") {
				spellings.push(Path::new(DATA_VOLUME).join(rest));
			}
		}
	}

	spellings
}

/// Whether a path is inside this process's own data directory, under any of
/// its spellings.
pub fn is_own_data(path: &Path) -> bool {
	OWN_DATA_DIRS
		.get()
		.is_some_and(|own| own.iter().any(|dir| path.starts_with(dir)))
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

#[cfg(test)]
mod tests {
	use super::*;

	/// The data directory must be recognized under every firmlink spelling.
	/// Watcher events arrive under `/System/Volumes/Data` while the
	/// configured path uses the `/Users` alias; missing either spelling lets
	/// the daemon's own SQLite journals dirty the source they live under,
	/// and the nudged hashing job then feeds the watcher forever.
	#[test]
	#[cfg(target_os = "macos")]
	fn data_dir_is_recognized_under_both_firmlink_spellings() {
		let spellings = path_spellings(Path::new("/Users/someone/.spacedrive"));
		assert!(spellings.contains(&PathBuf::from("/Users/someone/.spacedrive")));
		assert!(spellings.contains(&PathBuf::from(
			"/System/Volumes/Data/Users/someone/.spacedrive"
		)));

		let spellings = path_spellings(Path::new("/System/Volumes/Data/Users/someone/.spacedrive"));
		assert!(spellings.contains(&PathBuf::from("/Users/someone/.spacedrive")));
	}
}
