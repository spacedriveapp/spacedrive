use serde::{Deserialize, Serialize};

/// Defines what a memory file is scoped to
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MemoryScope {
	/// Attached to a specific directory
	Directory { path: String },

	/// Scoped to an entire project/repository
	Project { root_path: String },

	/// Topic-based (not tied to location)
	Topic { topic: String },

	/// Standalone portable memory
	Standalone,
}

impl MemoryScope {
	pub fn as_str(&self) -> &'static str {
		match self {
			Self::Directory { .. } => "directory",
			Self::Project { .. } => "project",
			Self::Topic { .. } => "topic",
			Self::Standalone => "standalone",
		}
	}

	/// Get the scope identifier for display
	pub fn identifier(&self) -> String {
		match self {
			Self::Directory { path } => path.clone(),
			Self::Project { root_path } => root_path.clone(),
			Self::Topic { topic } => topic.clone(),
			Self::Standalone => "standalone".to_string(),
		}
	}
}
