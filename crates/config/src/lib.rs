use anyhow::{anyhow, Result};
use std::fs;
use std::path::PathBuf;

/// Platform-specific data directory resolution
pub fn default_data_dir() -> Result<PathBuf> {
	#[cfg(target_os = "macos")]
	let dir = dirs::data_dir()
		.ok_or_else(|| anyhow!("Could not determine data directory"))?
		.join("spacedrive");

	#[cfg(target_os = "windows")]
	let dir = dirs::data_dir()
		.ok_or_else(|| anyhow!("Could not determine data directory"))?
		.join("Spacedrive");

	#[cfg(target_os = "linux")]
	let dir = dirs::data_local_dir()
		.ok_or_else(|| anyhow!("Could not determine data directory"))?
		.join("spacedrive");

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
