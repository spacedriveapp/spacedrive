//! Input types for file rename operations

use crate::domain::addressing::SdPath;
use serde::{Deserialize, Serialize};
use specta::Type;

/// Input for renaming a file or directory
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct FileRenameInput {
	/// The file or directory to rename
	pub target: SdPath,
	/// The new name (filename only, no path separators)
	pub new_name: String,
}

impl FileRenameInput {
	/// Create a new file rename input
	pub fn new(target: SdPath, new_name: impl Into<String>) -> Self {
		Self {
			target,
			new_name: new_name.into(),
		}
	}
}
