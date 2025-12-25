use file_opening::{FileOpener, OpenResult, OpenWithApp};
use std::path::{Path, PathBuf};

pub struct LinuxFileOpener;

impl FileOpener for LinuxFileOpener {
	fn get_apps_for_file(&self, _path: &Path) -> Result<Vec<OpenWithApp>, String> {
		// Simple implementation - return empty list
		// Full implementation would require parsing freedesktop.org desktop entries
		Ok(vec![])
	}

	fn open_with_default(&self, path: &Path) -> Result<OpenResult, String> {
		if !path.exists() {
			return Ok(OpenResult::FileNotFound {
				path: path.to_string_lossy().to_string(),
			});
		}

		match open::that(path) {
			Ok(_) => Ok(OpenResult::Success),
			Err(e) => Ok(OpenResult::PlatformError {
				message: e.to_string(),
			}),
		}
	}

	fn open_with_app(&self, path: &Path, app_id: &str) -> Result<OpenResult, String> {
		if !path.exists() {
			return Ok(OpenResult::FileNotFound {
				path: path.to_string_lossy().to_string(),
			});
		}

		// Use xdg-open with specific app
		let output = std::process::Command::new("gtk-launch")
			.arg(app_id)
			.arg(path)
			.output()
			.map_err(|e| e.to_string())?;

		if output.status.success() {
			Ok(OpenResult::Success)
		} else {
			Ok(OpenResult::PlatformError {
				message: String::from_utf8_lossy(&output.stderr).to_string(),
			})
		}
	}

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
