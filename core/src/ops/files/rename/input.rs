//! Input types for rename operations

use crate::domain::addressing::SdPath;
use serde::{Deserialize, Serialize};
use specta::Type;

use super::rules::RenameRule;

/// Rename one file or directory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct FileRenameInput {
	/// The file or directory to rename
	pub target: SdPath,
	/// The new name (filename only, no path separators)
	pub new_name: String,
}

impl FileRenameInput {
	pub fn new(target: SdPath, new_name: impl Into<String>) -> Self {
		Self {
			target,
			new_name: new_name.into(),
		}
	}
}

/// Rename several files by rules applied to each name in order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct FileRenameBatchInput {
	pub targets: Vec<SdPath>,
	pub rules: Vec<RenameRule>,
}

impl FileRenameBatchInput {
	pub fn validate(&self) -> Result<(), Vec<String>> {
		let mut errors = Vec::new();
		if self.targets.is_empty() {
			errors.push("name at least one file to rename".to_string());
		}
		if self.rules.is_empty() {
			errors.push("give at least one rule".to_string());
		}
		if let Err(error) = super::rules::check_rules(&self.rules) {
			errors.push(error.to_string());
		}
		if errors.is_empty() {
			Ok(())
		} else {
			Err(errors)
		}
	}
}
