//! Get library configuration query

use crate::{
	context::CoreContext,
	infra::query::{LibraryQuery, QueryError, QueryResult},
	library::config::{IndexerSettings, LibrarySettings},
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

/// Input for getting library configuration
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetLibraryConfigQueryInput;

/// Library settings output
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LibrarySettingsOutput {
	/// Whether to generate thumbnails for media files
	pub generate_thumbnails: bool,

	/// Thumbnail quality (0-100)
	pub thumbnail_quality: u8,

	/// Whether to enable AI-powered tagging
	pub enable_ai_tagging: bool,

	/// Whether sync is enabled for this library
	pub sync_enabled: bool,

	/// Whether the library is encrypted at rest
	pub encryption_enabled: bool,

	/// Whether to automatically track system volumes
	pub auto_track_system_volumes: bool,

	/// Whether to automatically track external volumes when connected
	pub auto_track_external_volumes: bool,

	/// Indexer settings
	pub indexer: IndexerSettingsOutput,
}

/// Indexer settings output
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct IndexerSettingsOutput {
	/// Skip system files
	pub no_system_files: bool,

	/// Skip .git repositories
	pub no_git: bool,

	/// Skip dev directories (node_modules, etc.)
	pub no_dev_dirs: bool,

	/// Skip hidden files
	pub no_hidden: bool,

	/// Respect .gitignore files
	pub gitignore: bool,

	/// Only index images
	pub only_images: bool,
}

impl From<&LibrarySettings> for LibrarySettingsOutput {
	fn from(settings: &LibrarySettings) -> Self {
		Self {
			generate_thumbnails: settings.generate_thumbnails,
			thumbnail_quality: settings.thumbnail_quality,
			enable_ai_tagging: settings.enable_ai_tagging,
			sync_enabled: settings.sync_enabled,
			encryption_enabled: settings.encryption_enabled,
			auto_track_system_volumes: settings.auto_track_system_volumes,
			auto_track_external_volumes: settings.auto_track_external_volumes,
			indexer: IndexerSettingsOutput::from(&settings.indexer),
		}
	}
}

impl From<&IndexerSettings> for IndexerSettingsOutput {
	fn from(settings: &IndexerSettings) -> Self {
		Self {
			no_system_files: settings.no_system_files,
			no_git: settings.no_git,
			no_dev_dirs: settings.no_dev_dirs,
			no_hidden: settings.no_hidden,
			gitignore: settings.gitignore,
			only_images: settings.only_images,
		}
	}
}

/// Query to get library configuration
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetLibraryConfigQuery;

impl LibraryQuery for GetLibraryConfigQuery {
	type Input = GetLibraryConfigQueryInput;
	type Output = LibrarySettingsOutput;

	fn from_input(_input: Self::Input) -> QueryResult<Self> {
		Ok(Self)
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let library_id = session
			.current_library_id
			.ok_or_else(|| QueryError::Internal("No library selected".to_string()))?;

		let library = context
			.libraries()
			.await
			.get_library(library_id)
			.await
			.ok_or_else(|| QueryError::Internal("Library not found".to_string()))?;

		let config = library.config().await;

		Ok(LibrarySettingsOutput::from(&config.settings))
	}
}

crate::register_library_query!(GetLibraryConfigQuery, "config.library.get");
