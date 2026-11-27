use std::path::{Path, PathBuf};
use tracing::error;

/// Reveal a file in the native file manager (Finder on macOS, Explorer on Windows, etc.)
#[tauri::command]
pub async fn reveal_file(path: String) -> Result<(), String> {
	let path = PathBuf::from(path);

	if !path.exists() {
		return Err(format!("Path does not exist: {}", path.display()));
	}

	reveal_path(&path).map_err(|e| {
		error!("Failed to reveal file: {:#?}", e);
		format!("Failed to reveal file: {}", e)
	})
}

/// Share files using the native system share sheet (macOS/iOS only)
#[tauri::command]
#[cfg_attr(not(target_os = "macos"), allow(unused_variables))]
pub async fn share_files(paths: Vec<String>) -> Result<(), String> {
	#[cfg(target_os = "macos")]
	{
		// Verify all paths exist
		for path in &paths {
			let path_buf = PathBuf::from(path);
			if !path_buf.exists() {
				return Err(format!("Path does not exist: {}", path));
			}
		}

		// Join paths with null separator (similar to open_file_paths_with pattern)
		let joined_paths = paths.join("\0");

		unsafe {
			let success = sd_desktop_macos::share_items(&joined_paths.as_str().into());
			if success {
				Ok(())
			} else {
				Err("Failed to show share sheet".to_string())
			}
		}
	}

	#[cfg(not(target_os = "macos"))]
	{
		Err("Share sheet is only supported on macOS".to_string())
	}
}

#[cfg(target_os = "macos")]
fn reveal_path(path: &Path) -> Result<(), std::io::Error> {
	std::process::Command::new("open")
		.arg("-R")
		.arg(path)
		.spawn()?
		.wait()?;
	Ok(())
}

#[cfg(target_os = "windows")]
fn reveal_path(path: &Path) -> Result<(), std::io::Error> {
	std::process::Command::new("explorer")
		.arg("/select,")
		.arg(path)
		.spawn()?
		.wait()?;
	Ok(())
}

#[cfg(target_os = "linux")]
fn reveal_path(path: &Path) -> Result<(), std::io::Error> {
	// On Linux, we'll try to open the parent directory
	// Different desktop environments have different file managers
	if let Some(parent) = path.parent() {
		std::process::Command::new("xdg-open")
			.arg(parent)
			.spawn()?
			.wait()?;
	}
	Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn reveal_path(path: &Path) -> Result<(), std::io::Error> {
	Err(std::io::Error::new(
		std::io::ErrorKind::Unsupported,
		"Reveal is not supported on this platform",
	))
}
