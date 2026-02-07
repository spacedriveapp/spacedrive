//! Location add action handler

use super::output::LocationAddOutput;
use crate::{
	context::CoreContext,
	infra::action::{
		context::ActionContextProvider,
		error::{ActionError, ActionResult},
		LibraryAction,
	},
	infra::db::entities,
	location::manager::LocationManager,
	ops::indexing::IndexMode,
};
use async_trait::async_trait;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use serde_json::json;
use specta::Type;
use std::{
	path::{Path, PathBuf},
	sync::Arc,
};
use tracing::warn;
use uuid::Uuid;

/// Safely canonicalize a path, with fallback for paths that can't be fully resolved.
/// This handles cases where the path exists but can't be canonicalized (e.g., on some
/// Android file systems or when intermediate directories have restrictive permissions).
fn safe_canonicalize(path: &Path) -> Result<PathBuf, ActionError> {
	// Try full canonicalization first
	match path.canonicalize() {
		Ok(canonical) => Ok(canonical),
		Err(e) => {
			// Try partial resolution: canonicalize parent + filename
			if let (Some(parent), Some(name)) = (path.parent(), path.file_name()) {
				if let Ok(canonical_parent) = parent.canonicalize() {
					let partial = canonical_parent.join(name);
					warn!(
						"Using partially canonicalized path: {} (full canonicalization failed: {})",
						partial.display(),
						e
					);
					return Ok(partial);
				}
			}

			// If path exists, use it as-is with a warning
			if path.exists() {
				warn!(
					"Using non-canonical path: {} (canonicalization failed: {})",
					path.display(),
					e
				);
				Ok(path.to_path_buf())
			} else {
				Err(ActionError::Validation {
					field: "path".to_string(),
					message: format!("Cannot resolve path: {}", e),
				})
			}
		}
	}
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LocationAddInput {
	pub path: crate::domain::addressing::SdPath,
	pub name: Option<String>,
	pub mode: IndexMode,
	pub job_policies: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationAddAction {
	input: LocationAddInput,
}

impl LocationAddAction {
	pub fn new(input: LocationAddInput) -> Self {
		Self { input }
	}
}

// Implement the new modular ActionType trait
impl LibraryAction for LocationAddAction {
	type Input = LocationAddInput;
	type Output = LocationAddOutput;

	fn from_input(input: LocationAddInput) -> Result<Self, String> {
		Ok(LocationAddAction::new(input))
	}

	async fn execute(
		self,
		library: std::sync::Arc<crate::library::Library>,
		context: std::sync::Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		// Get the device UUID from the device manager
		let device_uuid = context
			.device_manager
			.device_id()
			.map_err(ActionError::device_manager_error)?;

		// Get device record from database to get the integer ID
		// Note: There's a theoretical race condition between this lookup and location creation.
		// If the device is deleted between these operations, the location creation will fail
		// with a foreign key constraint error. This is an acceptable failure mode as device
		// deletion during location creation is extremely rare.
		let db = library.db().conn();
		let device_record = entities::device::Entity::find()
			.filter(entities::device::Column::Uuid.eq(device_uuid))
			.one(db)
			.await
			.map_err(ActionError::SeaOrm)?
			.ok_or_else(|| ActionError::DeviceNotFound(device_uuid))?;

		// Canonicalize the path to match what was validated
		let normalized_path = match &self.input.path {
			crate::domain::addressing::SdPath::Physical { device_slug, path } => {
				let canonical = safe_canonicalize(path)?;
				crate::domain::addressing::SdPath::Physical {
					device_slug: device_slug.clone(),
					path: canonical,
				}
			}
			other => other.clone(),
		};

		// Add the location using LocationManager
		let location_manager = LocationManager::new(context.events.as_ref().clone());

		let location_mode = match self.input.mode {
			IndexMode::None => crate::location::IndexMode::None,
			IndexMode::Shallow => crate::location::IndexMode::Shallow,
			IndexMode::Content => crate::location::IndexMode::Content,
			IndexMode::Deep => crate::location::IndexMode::Deep,
		};

		// Create action context for job tracking
		let action_context = self.create_action_context();

		// Serialize job_policies to JSON string if provided
		let job_policies_json = self
			.input
			.job_policies
			.as_ref()
			.and_then(|jp| serde_json::to_string(jp).ok());

		let (location_id, job_id_string) = location_manager
			.add_location(
				library.clone(),
				normalized_path.clone(),
				self.input.name.clone(),
				device_record.id,
				location_mode,
				Some(action_context),
				job_policies_json,
				&context.volume_manager,
			)
			.await
			.map_err(|e| ActionError::Internal(e.to_string()))?;

		// Parse the job ID from the string returned by add_location
		let job_id = if !job_id_string.is_empty() {
			Some(
				Uuid::parse_str(&job_id_string)
					.map_err(|e| ActionError::Internal(format!("Failed to parse job ID: {}", e)))?,
			)
		} else {
			None
		};

		let mut output = LocationAddOutput::new(location_id, normalized_path, self.input.name);

		if let Some(job_id) = job_id {
			output = output.with_job_id(job_id);
		}

		Ok(output)
	}

	fn action_kind(&self) -> &'static str {
		"locations.add"
	}

	async fn validate(
		&self,
		library: &std::sync::Arc<crate::library::Library>,
		context: std::sync::Arc<crate::context::CoreContext>,
	) -> Result<crate::infra::action::ValidationResult, ActionError> {
		use crate::domain::addressing::SdPath;

		match &self.input.path {
			SdPath::Physical {
				device_slug: _,
				path,
			} => {
				// Safely canonicalize the path (handles Android and other edge cases)
				let canonical_path = safe_canonicalize(path)?;

				// Validate local filesystem path
				if !canonical_path.exists() {
					return Err(ActionError::Validation {
						field: "path".to_string(),
						message: "Path does not exist".to_string(),
					});
				}
				if !canonical_path.is_dir() {
					return Err(ActionError::Validation {
						field: "path".to_string(),
						message: "Path must be a directory".to_string(),
					});
				}

				// Verify read permissions by attempting to read the directory
				match tokio::fs::read_dir(&canonical_path).await {
					Ok(_) => {
						// Can read directory, permissions are sufficient
					}
					Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
						return Err(ActionError::Validation {
							field: "path".to_string(),
							message: format!(
								"Permission denied reading directory: {}",
								canonical_path.display()
							),
						});
					}
					Err(e) => {
						return Err(ActionError::Validation {
							field: "path".to_string(),
							message: format!("Cannot read directory: {}", e),
						});
					}
				}
			}
			SdPath::Cloud {
				service,
				identifier,
				path: cloud_path,
			} => {
				// Validate cloud path by looking up the volume using VolumeManager
				let _volume = context
					.volume_manager
					.find_cloud_volume(*service, identifier)
					.await
					.ok_or_else(|| ActionError::Validation {
						field: "cloud_volume".to_string(),
						message: format!(
							"Cloud volume not found: {}://{}",
							service.scheme(),
							identifier
						),
					})?;

				// TODO: Validate that the path exists on the cloud volume
				// This would require accessing the VolumeBackend, which isn't available in validation
				// For now, we trust the user's input
			}
			SdPath::Content { .. } => {
				return Err(ActionError::Validation {
					field: "path".to_string(),
					message: "Content paths cannot be used as locations".to_string(),
				});
			}
			SdPath::Sidecar { .. } => {
				return Err(ActionError::Validation {
					field: "path".to_string(),
					message: "Sidecar paths cannot be used as locations".to_string(),
				});
			}
		}

		// Check for duplicate locations
		// TODO: Implement proper duplicate detection for both Physical and Cloud paths

		Ok(crate::infra::action::ValidationResult::Success { metadata: None })
	}
}

impl ActionContextProvider for LocationAddAction {
	fn create_action_context(&self) -> crate::infra::action::context::ActionContext {
		use crate::infra::action::context::{sanitize_action_input, ActionContext};

		ActionContext::new(
			Self::action_type_name(),
			sanitize_action_input(&self.input),
			json!({
				"operation": "add_location",
				"trigger": "user_action",
				"path": self.input.path.to_string(),
				"name": self.input.name,
				"mode": self.input.mode
			}),
		)
	}

	fn action_type_name() -> &'static str
	where
		Self: Sized,
	{
		"locations.add"
	}
}

// Register action
crate::register_library_action!(LocationAddAction, "locations.add");
