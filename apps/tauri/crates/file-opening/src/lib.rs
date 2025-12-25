use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Represents an application that can open a file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenWithApp {
	/// Platform-specific identifier:
	/// - macOS: bundle ID (com.apple.Preview)
	/// - Windows: application name
	/// - Linux: desktop entry ID (org.gnome.Evince.desktop)
	pub id: String,

	/// Human-readable display name
	pub name: String,

	/// Optional: app icon as base64-encoded PNG (for future use)
	#[serde(skip_serializing_if = "Option::is_none")]
	pub icon: Option<String>,
}

/// Result of attempting to open a file
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum OpenResult {
	Success,
	FileNotFound { path: String },
	AppNotFound { app_id: String },
	PermissionDenied { path: String },
	PlatformError { message: String },
}

/// Trait for platform-specific file opening implementations
pub trait FileOpener: Send + Sync {
	/// Get list of applications that can open this file
	fn get_apps_for_file(&self, path: &Path) -> Result<Vec<OpenWithApp>, String>;

	/// Get list of apps that can open all provided files (intersection)
	fn get_apps_for_files(&self, paths: &[PathBuf]) -> Result<Vec<OpenWithApp>, String> {
		if paths.is_empty() {
			return Ok(vec![]);
		}

		// Get apps for first file
		let mut common_apps = self
			.get_apps_for_file(&paths[0])?
			.into_iter()
			.map(|app| (app.id.clone(), app))
			.collect::<HashMap<_, _>>();

		// Intersect with remaining files
		for path in &paths[1..] {
			let apps = self
				.get_apps_for_file(path)?
				.into_iter()
				.map(|app| app.id)
				.collect::<HashSet<_>>();

			common_apps.retain(|id, _| apps.contains(id));
		}

		let mut result: Vec<_> = common_apps.into_values().collect();
		result.sort_by(|a, b| a.name.cmp(&b.name));
		Ok(result)
	}

	/// Open file with system default application
	fn open_with_default(&self, path: &Path) -> Result<OpenResult, String>;

	/// Open file with specific application
	fn open_with_app(&self, path: &Path, app_id: &str) -> Result<OpenResult, String>;

	/// Open multiple files with specific application
	fn open_files_with_app(
		&self,
		paths: &[PathBuf],
		app_id: &str,
	) -> Result<Vec<OpenResult>, String> {
		paths
			.iter()
			.map(|path| self.open_with_app(path, app_id))
			.collect()
	}
}
