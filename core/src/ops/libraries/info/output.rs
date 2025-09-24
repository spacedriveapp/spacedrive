//! Library information output types

use crate::library::config::{LibrarySettings, LibraryStatistics};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;
use uuid::Uuid;

/// Detailed information about a library
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LibraryInfoOutput {
	/// Library unique identifier
	pub id: Uuid,

	/// Human-readable library name
	pub name: String,

	/// Optional description
	pub description: Option<String>,

	/// Path to the library directory
	pub path: PathBuf,

	/// When the library was created
	pub created_at: DateTime<Utc>,

	/// When the library was last modified
	pub updated_at: DateTime<Utc>,

	/// Library-specific settings
	pub settings: LibrarySettings,

	/// Library statistics
	pub statistics: LibraryStatistics,
}
