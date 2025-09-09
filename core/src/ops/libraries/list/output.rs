//! Library listing output types

use crate::library::config::LibraryStatistics;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/// Information about a library for listing purposes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryInfo {
	/// Library unique identifier
	pub id: Uuid,

	/// Human-readable library name
	pub name: String,

	/// Path to the library directory
	pub path: PathBuf,

	/// Optional statistics if requested
	pub stats: Option<LibraryStatistics>,
}

impl LibraryInfo {
	/// Create new library info
	pub fn new(id: Uuid, name: String, path: PathBuf, stats: Option<LibraryStatistics>) -> Self {
		Self {
			id,
			name,
			path,
			stats,
		}
	}
}
