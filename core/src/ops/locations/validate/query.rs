//! Query to validate location paths before adding them

use super::output::*;
use crate::{
	context::CoreContext,
	domain::addressing::SdPath,
	infra::query::{LibraryQuery, QueryError, QueryResult},
	volume::types::VolumeType,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{path::PathBuf, sync::Arc};

/// Input for location path validation
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ValidateLocationPathInput {
	pub path: SdPath,
}

/// Query to validate if a path is suitable for use as a location
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ValidateLocationPathQuery {
	input: ValidateLocationPathInput,
}

impl LibraryQuery for ValidateLocationPathQuery {
	type Input = ValidateLocationPathInput;
	type Output = ValidateLocationPathOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		_session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		// Cloud paths are always safe - no system directory concerns
		let path = match &self.input.path {
			SdPath::Physical { path, .. } => path,
			SdPath::Cloud { .. } => {
				return Ok(ValidateLocationPathOutput {
					is_recommended: true,
					risk_level: RiskLevel::Low,
					warnings: vec![],
					suggested_alternative: None,
					path_depth: 0,
					is_on_primary_volume: false,
				})
			}
			SdPath::Content { .. } | SdPath::Sidecar { .. } => {
				return Err(QueryError::Internal(
					"Content and Sidecar paths cannot be validated as locations".to_string(),
				))
			}
		};

		// Calculate path depth from root
		let depth = path.components().count() as u32;

		// Get volume information to determine if this is on the primary system volume
		let volume_manager = &context.volume_manager;
		let volume_opt = volume_manager.volume_for_path(path).await;

		tracing::info!(
			"Volume lookup for path {}: {:?}",
			path.display(),
			volume_opt.as_ref().map(|v| (
				v.name.as_str(),
				&v.volume_type,
				v.fingerprint.0.as_str()
			))
		);

		let is_primary = volume_opt
			.as_ref()
			.map(|v| v.volume_type == VolumeType::Primary)
			.unwrap_or(false);

		// Check if path matches known system directories
		let system_dirs = get_system_directories();
		tracing::info!("Validating path: {} (depth: {})", path.display(), depth);
		tracing::info!("System directories: {:?}", system_dirs);
		let is_system_dir = system_dirs.iter().any(|d| {
			// Special case: "/" should only match if path IS "/", not if it starts with "/"
			// Otherwise every absolute path would be considered a system directory
			let matches = if d.to_string_lossy() == "/" {
				path == d
			} else {
				path.starts_with(d)
			};
			if matches {
				tracing::info!("Path {} matches system dir {}", path.display(), d.display());
			}
			matches
		});
		tracing::info!(
			"is_system_dir: {}, is_primary: {}",
			is_system_dir,
			is_primary
		);

		// Determine risk level using hybrid approach (depth + system directory check)
		let risk_level = if is_system_dir || depth <= 1 {
			RiskLevel::High
		} else if depth == 2 && is_primary {
			RiskLevel::Medium
		} else {
			RiskLevel::Low
		};

		// Generate warnings and suggestions based on risk level
		let mut warnings = vec![];
		let mut suggested_alternative = None;

		match risk_level {
			RiskLevel::High => {
				if is_system_dir {
					warnings.push(ValidationWarning {
						message: "This is a system directory that contains OS files".to_string(),
						suggestion: Some(
							"Choose a user directory instead (like Documents or Downloads)"
								.to_string(),
						),
					});
				} else {
					warnings.push(ValidationWarning {
						message: "This path is at the root of your filesystem".to_string(),
						suggestion: Some(
							"Choose a more specific folder to avoid indexing system files"
								.to_string(),
						),
					});
				}

				// Suggest volume indexing for external volumes (not primary)
				if !is_primary {
					if let Some(vol) = volume_opt.as_ref() {
						suggested_alternative = Some(VolumeIndexingSuggestion {
							volume_fingerprint: vol.fingerprint.0.clone(),
							volume_name: vol.name.clone(),
							message: format!(
								"Consider using Volume Indexing for '{}' instead of adding it as a location",
								vol.name
							),
						});
					}
				}
			}
			RiskLevel::Medium => {
				warnings.push(ValidationWarning {
					message: "This is a high-level user directory".to_string(),
					suggestion: Some(
						"Consider selecting a specific subfolder (like Documents/Projects) instead"
							.to_string(),
					),
				});

				// Suggest volume indexing for external volumes
				if !is_primary {
					if let Some(vol) = volume_opt.as_ref() {
						suggested_alternative = Some(VolumeIndexingSuggestion {
							volume_fingerprint: vol.fingerprint.0.clone(),
							volume_name: vol.name.clone(),
							message: format!(
								"Or use Volume Indexing for '{}' to browse without adding a location",
								vol.name
							),
						});
					}
				}
			}
			RiskLevel::Low => {
				// No warnings needed for low-risk paths
			}
		}

		Ok(ValidateLocationPathOutput {
			is_recommended: risk_level == RiskLevel::Low,
			risk_level,
			warnings,
			suggested_alternative,
			path_depth: depth,
			is_on_primary_volume: is_primary,
		})
	}
}

/// Get platform-specific system directories that should not be added as locations
fn get_system_directories() -> Vec<PathBuf> {
	#[cfg(target_os = "macos")]
	{
		vec![
			PathBuf::from("/"),
			PathBuf::from("/System"),
			PathBuf::from("/Library"),
			PathBuf::from("/Applications"),
			PathBuf::from("/private"),
			PathBuf::from("/usr"),
			PathBuf::from("/bin"),
			PathBuf::from("/sbin"),
			PathBuf::from("/var"),
			PathBuf::from("/tmp"),
			PathBuf::from("/cores"),
		]
	}

	#[cfg(target_os = "linux")]
	{
		vec![
			PathBuf::from("/"),
			PathBuf::from("/bin"),
			PathBuf::from("/boot"),
			PathBuf::from("/dev"),
			PathBuf::from("/etc"),
			PathBuf::from("/lib"),
			PathBuf::from("/lib64"),
			PathBuf::from("/proc"),
			PathBuf::from("/root"),
			PathBuf::from("/run"),
			PathBuf::from("/sbin"),
			PathBuf::from("/sys"),
			PathBuf::from("/usr"),
			PathBuf::from("/var"),
			PathBuf::from("/tmp"),
		]
	}

	#[cfg(target_os = "windows")]
	{
		vec![
			PathBuf::from("C:\\"),
			PathBuf::from("C:\\Windows"),
			PathBuf::from("C:\\Program Files"),
			PathBuf::from("C:\\Program Files (x86)"),
			PathBuf::from("C:\\ProgramData"),
			PathBuf::from("C:\\System Volume Information"),
		]
	}

	#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
	{
		vec![]
	}
}
