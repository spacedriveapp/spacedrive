//! Query to validate a path before adding it as a source

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

/// Input for source path validation
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ValidateSourcePathInput {
	pub path: SdPath,
}

/// Query to validate whether a path is a sensible source root
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ValidateSourcePathQuery {
	input: ValidateSourcePathInput,
}

impl LibraryQuery for ValidateSourcePathQuery {
	type Input = ValidateSourcePathInput;
	type Output = ValidateSourcePathOutput;

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
				return Ok(ValidateSourcePathOutput {
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
					"Content and Sidecar paths cannot be source roots".to_string(),
				))
			}
		};

		// Calculate path depth from root
		let depth = path.components().count() as u32;

		// Get volume information to determine if this is on the primary system volume
		let volume_manager = &context.volume_manager;
		let volume_opt = volume_manager.volume_for_path(path).await;

		let is_primary = volume_opt
			.as_ref()
			.map(|v| v.volume_type == VolumeType::Primary)
			.unwrap_or(false);

		// Check if path matches known system directories
		let system_dirs = get_system_directories();
		let is_system_dir = system_dirs.iter().any(|d| {
			// Root paths (Unix "/" or Windows "C:\") must match exactly.
			// Using starts_with() on a root dir would flag every child path as a
			// system directory — e.g. "C:\" would match "C:\Users\alice\OneDrive".
			let d_is_root = d.parent().is_none();
			let matches = if d_is_root {
				path == d
			} else {
				path.starts_with(d)
			};
			matches
		});
		tracing::debug!(path = %path.display(), depth, is_system_dir, is_primary, "validated source path");

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
						suggested_alternative = Some(WholeVolumeSuggestion {
							volume_fingerprint: vol.fingerprint.0.clone(),
							volume_name: vol.name.clone(),
							message: format!("Consider tracking all of '{}' instead", vol.name),
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
						suggested_alternative = Some(WholeVolumeSuggestion {
							volume_fingerprint: vol.fingerprint.0.clone(),
							volume_name: vol.name.clone(),
							message: format!("Or track all of '{}' as one source", vol.name),
						});
					}
				}
			}
			RiskLevel::Low => {
				// No warnings needed for low-risk paths
			}
		}

		Ok(ValidateSourcePathOutput {
			is_recommended: risk_level == RiskLevel::Low,
			risk_level,
			warnings,
			suggested_alternative,
			path_depth: depth,
			is_on_primary_volume: is_primary,
		})
	}
}

/// Platform system directories that should not become source roots
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

crate::register_library_query!(ValidateSourcePathQuery, "sources.validate_path");
