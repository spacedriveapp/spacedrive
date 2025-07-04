//! Action System - User-initiated operations with audit logging
//!
//! This module provides a centralized, robust, and extensible layer for handling
//! all user-initiated operations. It serves as the primary integration point
//! for the CLI and future APIs.

use crate::shared::types::SdPath;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

pub mod error;
pub mod handler;
pub mod handlers;
pub mod manager;
pub mod receipt;
pub mod registry;
#[cfg(test)]
mod tests;

// Import handlers to trigger their registration
use handlers::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyOptions {
	pub overwrite: bool,
	pub preserve_attributes: bool,
	pub verify_integrity: bool,
}

impl Default for CopyOptions {
	fn default() -> Self {
		Self {
			overwrite: false,
			preserve_attributes: true,
			verify_integrity: true,
		}
	}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteOptions {
	pub permanent: bool,
	pub recursive: bool,
}

impl Default for DeleteOptions {
	fn default() -> Self {
		Self {
			permanent: false,
			recursive: false,
		}
	}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IndexMode {
	Shallow,
	Deep,
	Sync,
}

impl Default for IndexMode {
	fn default() -> Self {
		Self::Deep
	}
}

/// Represents a user-initiated action within Spacedrive.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Action {
	// Job-based file operations
	FileCopy {
		sources: Vec<SdPath>,
		destination: SdPath,
		options: CopyOptions,
	},

	FileDelete {
		targets: Vec<SdPath>,
		options: DeleteOptions,
	},

	// Direct (non-job) actions
	LibraryCreate {
		name: String,
		path: Option<PathBuf>,
	},

	LibraryDelete {
		library_id: Uuid,
	},

	// Hybrid actions (direct action that dispatches a job)
	LocationAdd {
		library_id: Uuid,
		path: PathBuf,
		name: Option<String>,
		mode: IndexMode,
	},

	LocationRemove {
		library_id: Uuid,
		location_id: Uuid,
	},

	LocationIndex {
		library_id: Uuid,
		location_id: Uuid,
		mode: IndexMode,
	},
}

impl Action {
	/// Returns a string identifier for the action type.
	pub fn kind(&self) -> &'static str {
		match self {
			Action::FileCopy { .. } => "file.copy",
			Action::FileDelete { .. } => "file.delete",
			Action::LibraryCreate { .. } => "library.create",
			Action::LibraryDelete { .. } => "library.delete",
			Action::LocationAdd { .. } => "location.add",
			Action::LocationRemove { .. } => "location.remove",
			Action::LocationIndex { .. } => "location.index",
		}
	}

	/// Returns a human-readable description of the action
	pub fn description(&self) -> String {
		match self {
			Action::FileCopy {
				sources,
				destination,
				..
			} => {
				format!(
					"Copy {} file(s) to {}",
					sources.len(),
					destination.display()
				)
			}
			Action::FileDelete { targets, .. } => {
				format!("Delete {} file(s)", targets.len())
			}
			Action::LibraryCreate { name, .. } => {
				format!("Create library '{}'", name)
			}
			Action::LibraryDelete { library_id } => {
				format!("Delete library {}", library_id)
			}
			Action::LocationAdd { path, name, .. } => match name {
				Some(name) => format!("Add location '{}' at {}", name, path.display()),
				None => format!("Add location at {}", path.display()),
			},
			Action::LocationRemove { location_id, .. } => {
				format!("Remove location {}", location_id)
			}
			Action::LocationIndex {
				location_id, mode, ..
			} => {
				format!("Index location {} ({:?})", location_id, mode)
			}
		}
	}

	/// Returns target summary for audit logging
	pub fn targets_summary(&self) -> serde_json::Value {
		match self {
			Action::FileCopy {
				sources,
				destination,
				..
			} => serde_json::json!({
				"sources": sources.iter().map(|s| s.display()).collect::<Vec<_>>(),
				"destination": destination.display()
			}),
			Action::FileDelete { targets, .. } => serde_json::json!({
				"targets": targets.iter().map(|t| t.display()).collect::<Vec<_>>()
			}),
			Action::LibraryCreate { name, path } => serde_json::json!({
				"name": name,
				"path": path.as_ref().map(|p| p.display().to_string())
			}),
			Action::LibraryDelete { library_id } => serde_json::json!({
				"library_id": library_id
			}),
			Action::LocationAdd {
				path, name, mode, ..
			} => serde_json::json!({
				"path": path.display().to_string(),
				"name": name,
				"mode": mode
			}),
			Action::LocationRemove { location_id, .. } => serde_json::json!({
				"location_id": location_id
			}),
			Action::LocationIndex {
				location_id, mode, ..
			} => serde_json::json!({
				"location_id": location_id,
				"mode": mode
			}),
		}
	}
}
