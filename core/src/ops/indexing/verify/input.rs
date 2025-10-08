//! Input types for index verification

use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct IndexVerifyInput {
	/// Path to verify (can be a location root or subdirectory)
	pub path: PathBuf,

	/// Whether to check content hashes (slower but more thorough)
	#[serde(default)]
	pub verify_content: bool,

	/// Whether to include detailed file-by-file comparison
	#[serde(default = "default_true")]
	pub detailed_report: bool,

	/// Whether to fix issues automatically (future feature)
	#[serde(default)]
	pub auto_fix: bool,
}

fn default_true() -> bool {
	true
}

impl IndexVerifyInput {
	pub fn new(path: PathBuf) -> Self {
		Self {
			path,
			verify_content: false,
			detailed_report: true,
			auto_fix: false,
		}
	}

	pub fn validate(&self) -> Result<(), Vec<String>> {
		let mut errors = Vec::new();

		if !self.path.exists() {
			errors.push(format!("Path does not exist: {}", self.path.display()));
		}

		if errors.is_empty() {
			Ok(())
		} else {
			Err(errors)
		}
	}
}
