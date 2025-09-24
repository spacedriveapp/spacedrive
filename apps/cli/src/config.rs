//! CLI-specific configuration management

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/// CLI configuration stored in the data directory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliConfig {
	/// Current library ID
	pub current_library_id: Option<Uuid>,
}

impl Default for CliConfig {
	fn default() -> Self {
		Self {
			current_library_id: None,
		}
	}
}

impl CliConfig {
	/// Get the CLI config file path
	pub fn config_path(data_dir: &PathBuf) -> PathBuf {
		data_dir.join("cli.json")
	}

	/// Load CLI config from the data directory
	pub fn load(data_dir: &PathBuf) -> Result<Self> {
		let config_path = Self::config_path(data_dir);

		if config_path.exists() {
			let json = std::fs::read_to_string(&config_path)?;
			let config: CliConfig = serde_json::from_str(&json)?;
			Ok(config)
		} else {
			// Create default config if it doesn't exist
			let config = Self::default();
			config.save(data_dir)?;
			Ok(config)
		}
	}

	/// Save CLI config to the data directory
	pub fn save(&self, data_dir: &PathBuf) -> Result<()> {
		// Ensure directory exists
		std::fs::create_dir_all(data_dir)?;

		let config_path = Self::config_path(data_dir);
		let json = serde_json::to_string_pretty(self)?;
		std::fs::write(&config_path, json)?;
		Ok(())
	}

	/// Set the current library ID and save
	pub fn set_current_library(&mut self, library_id: Uuid, data_dir: &PathBuf) -> Result<()> {
		self.current_library_id = Some(library_id);
		self.save(data_dir)
	}

	/// Clear the current library ID and save
	pub fn clear_current_library(&mut self, data_dir: &PathBuf) -> Result<()> {
		self.current_library_id = None;
		self.save(data_dir)
	}
}
