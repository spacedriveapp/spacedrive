use file_opening::{FileOpener, OpenResult, OpenWithApp};
use std::path::{Path, PathBuf};
use swift_rs::*;

swift!(fn get_apps_for_path(path: &SRString) -> SRString);
swift!(fn open_path_with_default(path: &SRString) -> SRString);
swift!(fn open_path_with_app(path: &SRString, app_id: &SRString) -> SRString);
swift!(fn open_paths_with_app(paths: &SRString, app_id: &SRString) -> SRString);

pub struct MacFileOpener;

impl FileOpener for MacFileOpener {
	fn get_apps_for_file(&self, path: &Path) -> Result<Vec<OpenWithApp>, String> {
		let path_str = path.to_string_lossy().to_string();
		let sr_path = SRString::from(path_str.as_str());

		unsafe {
			let result = get_apps_for_path(&sr_path).to_string();
			serde_json::from_str(&result).map_err(|e| e.to_string())
		}
	}

	fn open_with_default(&self, path: &Path) -> Result<OpenResult, String> {
		let path_str = path.to_string_lossy().to_string();
		let sr_path = SRString::from(path_str.as_str());

		unsafe {
			let result = open_path_with_default(&sr_path).to_string();
			serde_json::from_str(&result).map_err(|e| e.to_string())
		}
	}

	fn open_with_app(&self, path: &Path, app_id: &str) -> Result<OpenResult, String> {
		let path_str = path.to_string_lossy().to_string();
		let sr_path = SRString::from(path_str.as_str());
		let sr_app_id = SRString::from(app_id);

		unsafe {
			let result = open_path_with_app(&sr_path, &sr_app_id).to_string();
			serde_json::from_str(&result).map_err(|e| e.to_string())
		}
	}

	fn open_files_with_app(
		&self,
		paths: &[PathBuf],
		app_id: &str,
	) -> Result<Vec<OpenResult>, String> {
		// Use null-delimited paths for multiple files
		let paths_str = paths
			.iter()
			.map(|p| p.to_string_lossy())
			.collect::<Vec<_>>()
			.join("\0");
		let sr_paths = SRString::from(paths_str.as_str());
		let sr_app_id = SRString::from(app_id);

		unsafe {
			let result = open_paths_with_app(&sr_paths, &sr_app_id).to_string();
			serde_json::from_str(&result).map_err(|e| e.to_string())
		}
	}
}
