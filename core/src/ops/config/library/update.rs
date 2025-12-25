//! Update library configuration action

use crate::{
	context::CoreContext,
	infra::action::{error::ActionError, LibraryAction, ValidationResult},
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;
use tracing::info;

/// Input for updating library configuration
/// All fields are optional for partial updates
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct UpdateLibraryConfigInput {
	// Media settings
	/// Whether to generate thumbnails for media files
	#[serde(skip_serializing_if = "Option::is_none")]
	pub generate_thumbnails: Option<bool>,

	/// Thumbnail quality (1-100)
	#[serde(skip_serializing_if = "Option::is_none")]
	pub thumbnail_quality: Option<u8>,

	/// Whether to enable AI-powered tagging
	#[serde(skip_serializing_if = "Option::is_none")]
	pub enable_ai_tagging: Option<bool>,

	// Sync & Security
	/// Whether sync is enabled for this library
	#[serde(skip_serializing_if = "Option::is_none")]
	pub sync_enabled: Option<bool>,

	/// Whether the library is encrypted at rest
	#[serde(skip_serializing_if = "Option::is_none")]
	pub encryption_enabled: Option<bool>,

	// Auto-tracking
	/// Whether to automatically track system volumes
	#[serde(skip_serializing_if = "Option::is_none")]
	pub auto_track_system_volumes: Option<bool>,

	/// Whether to automatically track external volumes when connected
	#[serde(skip_serializing_if = "Option::is_none")]
	pub auto_track_external_volumes: Option<bool>,

	// Indexer settings
	/// Skip system files
	#[serde(skip_serializing_if = "Option::is_none")]
	pub no_system_files: Option<bool>,

	/// Skip .git repositories
	#[serde(skip_serializing_if = "Option::is_none")]
	pub no_git: Option<bool>,

	/// Skip dev directories (node_modules, etc.)
	#[serde(skip_serializing_if = "Option::is_none")]
	pub no_dev_dirs: Option<bool>,

	/// Skip hidden files
	#[serde(skip_serializing_if = "Option::is_none")]
	pub no_hidden: Option<bool>,

	/// Respect .gitignore files
	#[serde(skip_serializing_if = "Option::is_none")]
	pub gitignore: Option<bool>,

	/// Only index images
	#[serde(skip_serializing_if = "Option::is_none")]
	pub only_images: Option<bool>,
}

/// Output for update library configuration action
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct UpdateLibraryConfigOutput {
	/// Whether the update was successful
	pub success: bool,

	/// Message describing the result
	pub message: String,
}

/// Action to update library configuration
pub struct UpdateLibraryConfigAction {
	input: UpdateLibraryConfigInput,
}

impl LibraryAction for UpdateLibraryConfigAction {
	type Input = UpdateLibraryConfigInput;
	type Output = UpdateLibraryConfigOutput;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		Ok(Self { input })
	}

	async fn validate(
		&self,
		_library: &Arc<crate::library::Library>,
		_context: Arc<CoreContext>,
	) -> Result<ValidationResult, ActionError> {
		// Validate thumbnail quality
		if let Some(quality) = self.input.thumbnail_quality {
			if quality == 0 || quality > 100 {
				return Err(ActionError::Validation {
					field: "thumbnail_quality".to_string(),
					message: "Thumbnail quality must be between 1 and 100".to_string(),
				});
			}
		}

		Ok(ValidationResult::Success)
	}

	async fn execute(
		self,
		library: Arc<crate::library::Library>,
		_context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let mut changes = Vec::new();

		library
			.update_config(|config| {
				let settings = &mut config.settings;

				if let Some(generate_thumbnails) = self.input.generate_thumbnails {
					if settings.generate_thumbnails != generate_thumbnails {
						settings.generate_thumbnails = generate_thumbnails;
						changes.push("generate_thumbnails");
					}
				}

				if let Some(thumbnail_quality) = self.input.thumbnail_quality {
					if settings.thumbnail_quality != thumbnail_quality {
						settings.thumbnail_quality = thumbnail_quality;
						changes.push("thumbnail_quality");
					}
				}

				if let Some(enable_ai_tagging) = self.input.enable_ai_tagging {
					if settings.enable_ai_tagging != enable_ai_tagging {
						settings.enable_ai_tagging = enable_ai_tagging;
						changes.push("enable_ai_tagging");
					}
				}

				if let Some(sync_enabled) = self.input.sync_enabled {
					if settings.sync_enabled != sync_enabled {
						settings.sync_enabled = sync_enabled;
						changes.push("sync_enabled");
					}
				}

				if let Some(encryption_enabled) = self.input.encryption_enabled {
					if settings.encryption_enabled != encryption_enabled {
						settings.encryption_enabled = encryption_enabled;
						changes.push("encryption_enabled");
					}
				}

				if let Some(auto_track_system_volumes) = self.input.auto_track_system_volumes {
					if settings.auto_track_system_volumes != auto_track_system_volumes {
						settings.auto_track_system_volumes = auto_track_system_volumes;
						changes.push("auto_track_system_volumes");
					}
				}

				if let Some(auto_track_external_volumes) = self.input.auto_track_external_volumes {
					if settings.auto_track_external_volumes != auto_track_external_volumes {
						settings.auto_track_external_volumes = auto_track_external_volumes;
						changes.push("auto_track_external_volumes");
					}
				}

				// Indexer settings
				if let Some(no_system_files) = self.input.no_system_files {
					if settings.indexer.no_system_files != no_system_files {
						settings.indexer.no_system_files = no_system_files;
						changes.push("no_system_files");
					}
				}

				if let Some(no_git) = self.input.no_git {
					if settings.indexer.no_git != no_git {
						settings.indexer.no_git = no_git;
						changes.push("no_git");
					}
				}

				if let Some(no_dev_dirs) = self.input.no_dev_dirs {
					if settings.indexer.no_dev_dirs != no_dev_dirs {
						settings.indexer.no_dev_dirs = no_dev_dirs;
						changes.push("no_dev_dirs");
					}
				}

				if let Some(no_hidden) = self.input.no_hidden {
					if settings.indexer.no_hidden != no_hidden {
						settings.indexer.no_hidden = no_hidden;
						changes.push("no_hidden");
					}
				}

				if let Some(gitignore) = self.input.gitignore {
					if settings.indexer.gitignore != gitignore {
						settings.indexer.gitignore = gitignore;
						changes.push("gitignore");
					}
				}

				if let Some(only_images) = self.input.only_images {
					if settings.indexer.only_images != only_images {
						settings.indexer.only_images = only_images;
						changes.push("only_images");
					}
				}
			})
			.await
			.map_err(|e| ActionError::Internal(format!("Failed to update config: {}", e)))?;

		if changes.is_empty() {
			return Ok(UpdateLibraryConfigOutput {
				success: true,
				message: "No changes to apply".to_string(),
			});
		}

		info!(
			library_id = %library.id(),
			changes = ?changes,
			"Library configuration updated"
		);

		Ok(UpdateLibraryConfigOutput {
			success: true,
			message: format!("Updated: {}", changes.join(", ")),
		})
	}

	fn action_kind(&self) -> &'static str {
		"config.library.update"
	}
}

crate::register_library_action!(UpdateLibraryConfigAction, "config.library.update");
