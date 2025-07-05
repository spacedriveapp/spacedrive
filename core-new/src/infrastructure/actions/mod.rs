//! Action System - User-initiated operations with audit logging
//!
//! This module provides a centralized, robust, and extensible layer for handling
//! all user-initiated operations. It serves as the primary integration point
//! for the CLI and future APIs.

use crate::shared::types::SdPath;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

pub mod builder;
pub mod error;
pub mod handler;
pub mod manager;
pub mod output;
pub mod receipt;
pub mod registry;
#[cfg(test)]
mod tests;


/// Represents a user-initiated action within Spacedrive.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Action {
	// Global actions (no library context)
	LibraryCreate(crate::operations::libraries::create::action::LibraryCreateAction),
	LibraryDelete(crate::operations::libraries::delete::action::LibraryDeleteAction),
	
	// Library-scoped actions (require library_id)
	FileCopy { 
		library_id: Uuid, 
		action: crate::operations::files::copy::action::FileCopyAction 
	},
	FileDelete { 
		library_id: Uuid, 
		action: crate::operations::files::delete::action::FileDeleteAction 
	},
	FileValidate { 
		library_id: Uuid, 
		action: crate::operations::files::validation::ValidationAction 
	},
	DetectDuplicates { 
		library_id: Uuid, 
		action: crate::operations::files::duplicate_detection::DuplicateDetectionAction 
	},
	
	LocationAdd { 
		library_id: Uuid, 
		action: crate::operations::locations::add::action::LocationAddAction 
	},
	LocationRemove { 
		library_id: Uuid, 
		action: crate::operations::locations::remove::action::LocationRemoveAction 
	},
	LocationIndex { 
		library_id: Uuid, 
		action: crate::operations::locations::index::action::LocationIndexAction 
	},
	
	Index { 
		library_id: Uuid, 
		action: crate::operations::indexing::action::IndexingAction 
	},
	
	GenerateThumbnails { 
		library_id: Uuid, 
		action: crate::operations::media::thumbnail::action::ThumbnailAction 
	},
	
	ContentAnalysis { 
		library_id: Uuid, 
		action: crate::operations::content::action::ContentAction 
	},
	
	MetadataOperation { 
		library_id: Uuid, 
		action: crate::operations::metadata::action::MetadataAction 
	},
}

impl Action {
	/// Returns the library ID for library-scoped actions
	pub fn library_id(&self) -> Option<Uuid> {
		match self {
			Action::LibraryCreate(_) | Action::LibraryDelete(_) => None,
			Action::FileCopy { library_id, .. } => Some(*library_id),
			Action::FileDelete { library_id, .. } => Some(*library_id),
			Action::FileValidate { library_id, .. } => Some(*library_id),
			Action::DetectDuplicates { library_id, .. } => Some(*library_id),
			Action::LocationAdd { library_id, .. } => Some(*library_id),
			Action::LocationRemove { library_id, .. } => Some(*library_id),
			Action::LocationIndex { library_id, .. } => Some(*library_id),
			Action::Index { library_id, .. } => Some(*library_id),
			Action::GenerateThumbnails { library_id, .. } => Some(*library_id),
			Action::ContentAnalysis { library_id, .. } => Some(*library_id),
			Action::MetadataOperation { library_id, .. } => Some(*library_id),
		}
	}

	/// Returns a string identifier for the action type.
	pub fn kind(&self) -> &'static str {
		match self {
			Action::LibraryCreate(_) => "library.create",
			Action::LibraryDelete(_) => "library.delete",
			Action::FileCopy { .. } => "file.copy",
			Action::FileDelete { .. } => "file.delete",
			Action::FileValidate { .. } => "file.validate",
			Action::DetectDuplicates { .. } => "file.detect_duplicates",
			Action::LocationAdd { .. } => "location.add",
			Action::LocationRemove { .. } => "location.remove",
			Action::LocationIndex { .. } => "location.index",
			Action::Index { .. } => "indexing.index",
			Action::GenerateThumbnails { .. } => "media.thumbnail",
			Action::ContentAnalysis { .. } => "content.analyze",
			Action::MetadataOperation { .. } => "metadata.extract",
		}
	}

	/// Returns a human-readable description of the action
	pub fn description(&self) -> String {
		match self {
			Action::LibraryCreate(action) => {
				format!("Create library '{}'", action.name)
			}
			Action::LibraryDelete(_action) => {
				"Delete library".to_string()
			}
			Action::FileCopy { action, .. } => {
				format!(
					"Copy {} file(s) to {}",
					action.sources.len(),
					action.destination.display()
				)
			}
			Action::FileDelete { action, .. } => {
				format!("Delete {} file(s)", action.targets.len())
			}
			Action::FileValidate { action, .. } => {
				format!("Validate {} file(s)", action.paths.len())
			}
			Action::DetectDuplicates { action, .. } => {
				format!("Detect duplicates in {} path(s)", action.paths.len())
			}
			Action::LocationAdd { action, .. } => match &action.name {
				Some(name) => format!("Add location '{}' at {}", name, action.path.display()),
				None => format!("Add location at {}", action.path.display()),
			},
			Action::LocationRemove { action, .. } => {
				format!("Remove location {}", action.location_id)
			}
			Action::LocationIndex { action, .. } => {
				format!("Index location {} ({:?})", action.location_id, action.mode)
			}
			Action::Index { action, .. } => {
				format!("Index {} path(s)", action.paths.len())
			}
			Action::GenerateThumbnails { action, .. } => {
				format!("Generate thumbnails for {} file(s)", action.paths.len())
			}
			Action::ContentAnalysis { action, .. } => {
				format!("Analyze content of {} file(s)", action.paths.len())
			}
			Action::MetadataOperation { action, .. } => {
				format!("Extract metadata from {} file(s)", action.paths.len())
			}
		}
	}

	/// Returns target summary for audit logging
	pub fn targets_summary(&self) -> serde_json::Value {
		match self {
			Action::LibraryCreate(action) => serde_json::json!({
				"name": action.name,
				"path": action.path.as_ref().map(|p| p.display().to_string())
			}),
			Action::LibraryDelete(_action) => serde_json::json!({}),
			Action::FileCopy { action, .. } => serde_json::json!({
				"sources": action.sources.iter().map(|s| s.display().to_string()).collect::<Vec<_>>(),
				"destination": action.destination.display().to_string()
			}),
			Action::FileDelete { action, .. } => serde_json::json!({
				"targets": action.targets.iter().map(|t| t.display().to_string()).collect::<Vec<_>>()
			}),
			Action::FileValidate { action, .. } => serde_json::json!({
				"paths": action.paths.iter().map(|p| p.display().to_string()).collect::<Vec<_>>()
			}),
			Action::DetectDuplicates { action, .. } => serde_json::json!({
				"paths": action.paths.iter().map(|p| p.display().to_string()).collect::<Vec<_>>()
			}),
			Action::LocationAdd { action, .. } => serde_json::json!({
				"path": action.path.display().to_string(),
				"name": action.name,
				"mode": action.mode
			}),
			Action::LocationRemove { action, .. } => serde_json::json!({
				"location_id": action.location_id
			}),
			Action::LocationIndex { action, .. } => serde_json::json!({
				"location_id": action.location_id,
				"mode": action.mode
			}),
			Action::Index { action, .. } => serde_json::json!({
				"paths": action.paths.iter().map(|p| p.display().to_string()).collect::<Vec<_>>()
			}),
			Action::GenerateThumbnails { action, .. } => serde_json::json!({
				"paths": action.paths.iter().map(|p| p.display().to_string()).collect::<Vec<_>>()
			}),
			Action::ContentAnalysis { action, .. } => serde_json::json!({
				"paths": action.paths.iter().map(|p| p.display().to_string()).collect::<Vec<_>>()
			}),
			Action::MetadataOperation { action, .. } => serde_json::json!({
				"paths": action.paths.iter().map(|p| p.display().to_string()).collect::<Vec<_>>()
			}),
		}
	}
}
