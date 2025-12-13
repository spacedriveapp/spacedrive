//! Library - a Spacedrive library (collection of indexed locations)
//!
//! Libraries are the top-level organizational unit in Spacedrive.
//! Each library has its own database, settings, and set of locations.

use crate::domain::resource::Identifiable;
use crate::library::config::{LibrarySettings, LibraryStatistics};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;
use uuid::Uuid;

/// A Spacedrive library - the canonical domain model
///
/// This is the resource type sent to the frontend for the normalized cache.
/// It contains all the information needed to display library info in the UI.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Library {
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

impl Library {
	/// Create a new Library from a LibraryConfig and path
	pub fn from_config(config: &crate::library::config::LibraryConfig, path: PathBuf) -> Self {
		Self {
			id: config.id,
			name: config.name.clone(),
			description: config.description.clone(),
			path,
			created_at: config.created_at,
			updated_at: config.updated_at,
			settings: config.settings.clone(),
			statistics: config.statistics.clone(),
		}
	}
}

impl Identifiable for Library {
	fn id(&self) -> Uuid {
		self.id
	}

	fn resource_type() -> &'static str {
		"library"
	}

	/// Libraries are special - they're not stored in a DB table but in config files.
	/// This queries the LibraryManager for the library data.
	///
	/// Note: This requires access to CoreContext, which isn't available here.
	/// For now, we return an empty vec and expect callers to use the EventEmitter
	/// trait methods directly with pre-constructed Library instances.
	async fn from_ids(
		_db: &sea_orm::DatabaseConnection,
		_ids: &[Uuid],
	) -> crate::common::errors::Result<Vec<Self>>
	where
		Self: Sized,
	{
		// Libraries are stored in config files, not in the database.
		// The ResourceManager handles library events specially via emit_changed().
		// See library/mod.rs for library event emission.
		Ok(vec![])
	}
}

// Register Library as a simple resource
// Note: from_ids returns empty vec - library events should use emit_changed() directly
crate::register_resource!(Library);

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_library_resource_type() {
		assert_eq!(Library::resource_type(), "library");
	}
}
